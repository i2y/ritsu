//! `ringi diff` (DESIGN 11.2, 11.4), held to what the reference interpreter answers: each example
//! changed one way at a time (a row of the route's table, the amount a row ends at, a stage taken
//! out, the condition of a shared route, a way to choose, a deadline, the range of a field, a
//! person of the organization moved), the two versions compared, and a form tried on both sides of
//! each boundary and at the ends of the range with `simulate`: where the two versions come to other
//! stages, the diff says so, with the stages it says; where they come to the same, it says nothing.
//! The approvers it names for a requester are those `simulate` gives. `--versions` is held to
//! `simulate` of every requester on the two days. What it writes is held to the files in
//! `tests/golden/diff/` (`RINGI_BLESS=1 cargo test -p ringi --test diff` writes them again).

mod common;

use ringi::check::{Options, Run};
use ringi::diff::{self, Diff, Side};
use ringi::org::resolve::Requester;
use ringi::simulate::{Answer, Ask, On, StageSim, simulate};
use ritsu_base::fs::Overlay;
use ritsu_base::text::Lang;
use ritsu_ports::DiffHeld;
use std::path::Path;
use std::rc::Rc;

/// A file without its examples and scenarios, which say what one version answers: both sides of
/// a variant are made from it, so that each passes its check.
fn stripped(src: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for l in src.lines() {
        if l.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            skipping = l == "examples" || l.starts_with("scenario ");
        }
        if !skipping {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

fn edited(src: &str, edits: &[(&str, &str)]) -> String {
    let mut s = src.to_string();
    for (a, b) in edits {
        assert_eq!(s.matches(a).count(), 1, "{a:?}");
        s = s.replace(a, b);
    }
    s
}

/// A value of a route's input a probe gives: a word of an enum, or a number.
#[derive(Clone, Copy)]
enum In {
    W(&'static str),
    N(i128),
}

/// One form tried on both versions: the form, and what the route reads of it, by the lowered rule's
/// names.
#[derive(Clone)]
struct Probe {
    form: String,
    reads: Vec<(&'static str, In)>,
}

/// A change of an example: the file, the version of the organization it is read in (`--at`), what
/// is changed in the new version (and, for data, in which file), and the forms to try.
struct Variant {
    name: &'static str,
    file: &'static str,
    at: Option<&'static str>,
    edits: Vec<(&'static str, &'static str)>,
    /// A file of the data the side with the change reads otherwise (the new side, or the old for one
    /// that undoes the change), and the change.
    data: Option<(&'static str, &'static str, &'static str)>,
    probes: Vec<Probe>,
    grid: Option<Grid>,
    /// The change undone: the old version is the one with the edits and the data changed, the new
    /// is the example as it is (what the edits take out comes back: a stage added).
    back: bool,
}

/// The two sides of a variant, each read through its own files: the old as the example is (its
/// examples and scenarios taken out), the new with the change; for one that undoes the change
/// (`back`), the old with the change and the new as the example is.
fn sides(v: &Variant) -> (Side, Side) {
    let src = std::fs::read_to_string(v.file).unwrap();
    let example = stripped(&src);
    let changed = edited(&example, &v.edits);
    let (old_src, new_src) = if v.back { (&changed, &example) } else { (&example, &changed) };
    let path = v.file.replace(".ringi", "_old.ringi");
    let new_path = v.file.replace(".ringi", "_new.ringi");
    let mut old = Overlay::new(ritsu_base::fs::current());
    old.put(&ritsu_base::paths::absolute(Path::new(&path)), old_src.as_str());
    let mut new = Overlay::new(ritsu_base::fs::current());
    new.put(&ritsu_base::paths::absolute(Path::new(&new_path)), new_src.as_str());
    if let Some((f, a, b)) = v.data {
        let d = std::fs::read_to_string(f).unwrap();
        let side = if v.back { &mut old } else { &mut new };
        side.put(&ritsu_base::paths::absolute(Path::new(f)), edited(&d, &[(a, b)]).as_str());
    }
    let shown = |p: &str| p.strip_prefix("examples/").unwrap_or(p).to_string();
    (Side { read: path.clone(), files: Some(Rc::new(old)), shown: shown(&path) }, Side { read: new_path.clone(), files: Some(Rc::new(new)), shown: shown(&new_path) })
}

fn options(at: Option<&str>) -> Options {
    Options { at: at.map(|d| ritsu_ports::day_of(d).unwrap()), ..Options::default() }
}

fn compared(v: &Variant, lang: Lang) -> Diff {
    let (o, n) = sides(v);
    diff::diff(&o, &n, &common::joined(), &options(v.at), None, lang, lang).unwrap_or_else(|e| panic!("{}: {e:?}", v.name))
}

/// The stages `simulate` gives each form on one side, each stage with how it is written, for the
/// requester e1201 on 2026-11-16.
fn stages(side: &Side, at: Option<&str>, probes: &[Probe]) -> Vec<Vec<(String, String)>> {
    let files = side.files.clone().unwrap();
    ritsu_base::fs::with(files, || {
        let run = Run::with(common::joined(), options(at));
        let o = run.file(&side.read).unwrap();
        assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
        let day = ritsu_ports::day_of("2026-11-16").unwrap();
        probes
            .iter()
            .map(|p| {
                let ask = Ask { requester: Some("e1201".to_string()), every: false, form: Some(p.form.clone()), on: On::Day(day) };
                let a = simulate(&o, &run, &ask).unwrap_or_else(|t| panic!("{}: {}", p.form, t.en));
                a.sims[0].stages.iter().map(|s| (s.stage.clone(), s.shown.clone())).collect()
            })
            .collect()
    })
}

/// The forms tried besides a variant's own: each word of one input with each number of another at
/// the ends of its range, and on both sides of each end of each box the diff says.
struct Grid {
    words: Vec<&'static str>,
    number: &'static str,
    ends: (i128, i128),
    make: fn(&'static str, i128) -> Probe,
}

impl Grid {
    fn probes(&self, d: &Diff) -> Vec<Probe> {
        let mut at = vec![self.ends.0, self.ends.1];
        for r in &d.route {
            for (name, h) in &r.held {
                let DiffHeld::Runs(runs) = h else { continue };
                if name != self.number {
                    continue;
                }
                for (lo, hi) in runs {
                    if let Some(l) = lo {
                        assert_eq!(l.den, 1, "{name}");
                        at.extend([l.num - 1, l.num]);
                    }
                    if let Some(h) = hi {
                        assert_eq!(h.den, 1, "{name}");
                        at.extend([h.num, h.num + 1]);
                    }
                }
            }
        }
        at.retain(|n| (self.ends.0..=self.ends.1).contains(n));
        at.sort();
        at.dedup();
        self.words.iter().flat_map(|w| at.iter().map(|n| (self.make)(w, *n))).collect()
    }
}

/// Whether a box holds the probe: each input the box narrows, the probe's value in it.
fn holds(held: &[(String, DiffHeld)], p: &Probe) -> bool {
    held.iter().all(|(name, h)| {
        let Some((_, v)) = p.reads.iter().find(|(n, _)| n == name) else { return true };
        match (h, v) {
            (DiffHeld::Any, _) => true,
            (DiffHeld::Words { words, .. }, In::W(w)) => words.iter().any(|x| x == w),
            (DiffHeld::Runs(runs), In::N(n)) => runs.iter().any(|(lo, hi)| lo.is_none_or(|l| l.num <= n * l.den) && hi.is_none_or(|h| n * h.den <= h.num)),
            _ => false,
        }
    })
}

fn expense(category: &'static str, amount: i128) -> Probe {
    let mut items = Vec::new();
    let mut left = amount;
    while left > 0 {
        let p = left.min(1_000_000);
        items.push(format!("{{what: \"x\", price: {p}JPY}}"));
        left -= p;
    }
    Probe { form: format!("category = {category}, items = [{}]", items.join(", ")), reads: vec![("category", In::W(category)), ("amount", In::N(amount))] }
}

fn leave(days: i128) -> Probe {
    Probe { form: format!("kind = sick, days = {days}"), reads: vec![("days", In::N(days))] }
}

fn contract(kind: &'static str, value: i128) -> Probe {
    Probe { form: format!("value = {value}JPY, kind = {kind}"), reads: vec![("value", In::N(value)), ("kind", In::W(kind))] }
}

fn contract_for(kind: &'static str, value: i128, months: i128) -> Probe {
    Probe { form: format!("value = {value}JPY, kind = {kind}, months = {months}"), reads: vec![("value", In::N(value)), ("kind", In::W(kind)), ("months", In::N(months))] }
}

fn account(level: &'static str) -> Probe {
    Probe { form: format!("level = {level}"), reads: vec![("level", In::W(level))] }
}

fn variants() -> Vec<Variant> {
    vec![
        Variant {
            name: "expense_row",
            file: "examples/expense.ringi",
            at: None,
            edits: vec![("| expense  | >100_000JPY <=1_000_000JPY | check       |", "| expense  | >100_000JPY <=1_000_000JPY |             |"), ("request expense v1", "request expense v2")],
            data: None,
            probes: vec![
                expense("expense", 100_000),
                expense("expense", 100_001),
                expense("expense", 1_000_000),
                expense("expense", 1_000_001),
                expense("travel", 100_001),
                expense("purchase", 1),
                expense("expense", 1),
                expense("expense", 10_000_000),
            ],
            grid: Some(Grid { words: vec!["travel", "expense", "purchase"], number: "amount", ends: (1, 10_000_000), make: expense }),
            back: false,
        },
        Variant {
            name: "expense_threshold",
            file: "examples/expense.ringi",
            at: None,
            edits: vec![("| expense  | <=100_000JPY               |", "| expense  | <=50_000JPY                |"), ("| expense  | >100_000JPY <=1_000_000JPY |", "| expense  | >50_000JPY <=1_000_000JPY  |"), ("request expense v1", "request expense v2")],
            data: None,
            probes: vec![
                expense("expense", 50_000),
                expense("expense", 50_001),
                expense("expense", 100_000),
                expense("expense", 100_001),
                expense("travel", 50_001),
                expense("purchase", 100_000),
                expense("expense", 1),
                expense("expense", 10_000_000),
            ],
            grid: Some(Grid { words: vec!["travel", "expense", "purchase"], number: "amount", ends: (1, 10_000_000), make: expense }),
            back: false,
        },
        Variant {
            name: "leave_condition",
            file: "examples/leave.ringi",
            at: None,
            edits: vec![("route standard(long: days > 5)", "route standard(long: days > 3)"), ("request leave v1", "request leave v2")],
            data: None,
            probes: vec![leave(1), leave(3), leave(4), leave(5), leave(6), leave(20)],
            grid: Some(Grid { words: vec!["sick"], number: "days", ends: (1, 20), make: |_, n| leave(n) }),
            back: false,
        },
        Variant {
            name: "contract_stage",
            file: "examples/contract.ringi",
            at: Some("2026-11-16"),
            edits: vec![("  second   at least 2 of legal  when value > 5_000_000JPY\n", ""), ("  stage second   \"二人目\"\n", ""), ("request contract v1", "request contract v2")],
            data: None,
            probes: vec![contract("service", 10_000_000), contract("service", 10_000_001)],
            grid: Some(Grid { words: vec!["nda", "service", "license"], number: "value", ends: (0, 1_000_000_000), make: contract }),
            back: false,
        },
        Variant {
            name: "contract_stage_added",
            file: "examples/contract.ringi",
            at: Some("2026-11-16"),
            edits: vec![("  second   at least 2 of legal  when value > 5_000_000JPY\n", ""), ("  stage second   \"二人目\"\n", "")],
            data: None,
            probes: vec![contract("service", 10_000_000), contract("service", 10_000_001)],
            grid: Some(Grid { words: vec!["nda", "service", "license"], number: "value", ends: (0, 1_000_000_000), make: contract }),
            back: true,
        },
        // a stage made conditional on a field the old route does not read: the two routes take
        // different inputs, and are compared case by case all the same
        Variant {
            name: "contract_new_field",
            file: "examples/contract.ringi",
            at: Some("2026-11-16"),
            edits: vec![("  review   reviewer\n", "  review   reviewer             when months > 12\n"), ("request contract v1", "request contract v2")],
            data: None,
            probes: vec![contract_for("service", 1_000_000, 12), contract_for("service", 1_000_000, 13), contract_for("nda", 0, 60)],
            grid: Some(Grid { words: vec!["nda", "service", "license"], number: "months", ends: (1, 60), make: |k, n| contract_for(k, 1_000_000, n) }),
            back: false,
        },
        // the same undone: the stage comes again whatever the term
        Variant {
            name: "contract_new_field_back",
            file: "examples/contract.ringi",
            at: Some("2026-11-16"),
            edits: vec![("  review   reviewer\n", "  review   reviewer             when months > 12\n")],
            data: None,
            probes: vec![contract_for("service", 1_000_000, 12), contract_for("service", 1_000_000, 13)],
            grid: Some(Grid { words: vec!["nda", "service", "license"], number: "months", ends: (1, 60), make: |k, n| contract_for(k, 1_000_000, n) }),
            back: true,
        },
        Variant {
            name: "account_way",
            file: "examples/account.ringi",
            at: None,
            edits: vec![("  security     = team it_security", "  security     = team it_operations")],
            data: None,
            probes: vec![account("read"), account("write"), account("admin")],
            grid: None,
            back: false,
        },
        Variant {
            name: "expense_deadline",
            file: "examples/expense.ringi",
            at: None,
            edits: vec![("  remind after 2 business days at 09:00", "  remind after 1 business day at 09:00"), ("request expense v1", "request expense v2")],
            data: None,
            probes: vec![expense("travel", 1), expense("expense", 100_001)],
            grid: None,
            back: false,
        },
        Variant {
            name: "expense_range",
            file: "examples/expense.ringi",
            at: None,
            edits: vec![("range >=1JPY <=1_000_000JPY", "range >=1JPY <=2_000_000JPY"), ("request expense v1", "request expense v2")],
            data: None,
            probes: vec![expense("expense", 1), expense("purchase", 10_000_000)],
            grid: None,
            back: false,
        },
        Variant {
            name: "expense_person_moved",
            file: "examples/expense.ringi",
            at: None,
            edits: vec![],
            data: Some(("examples/org/2026-04-01/members.csv", "e1201,sales1_a,staff,yes", "e1201,accounting,staff,yes")),
            probes: vec![expense("expense", 1), expense("expense", 100_001), expense("purchase", 1_000_001)],
            grid: None,
            back: false,
        },
    ]
}

/// Where the two versions come to other stages for a form, and only there, the diff says so; and
/// the stages it says change are the ones that do, from what to what.
#[test]
fn what_the_diff_says_of_the_route_is_what_simulate_gives() {
    let (mut tried, mut inside_any, mut boxes) = (0, 0, 0);
    for v in variants() {
        let d = compared(&v, Lang::En);
        boxes += d.route.len();
        // two routes that read different fields are compared case by case all the same
        if v.name.starts_with("contract_new_field") {
            assert!(d.total && d.unsettled.is_empty() && !d.route.is_empty(), "{}: {:?}", v.name, d.unsettled);
        }
        let (o, n) = sides(&v);
        let mut probes = v.probes.clone();
        probes.extend(v.grid.as_ref().map(|g| g.probes(&d)).unwrap_or_default());
        let (a, b) = (stages(&o, v.at, &probes), stages(&n, v.at, &probes));
        for (k, p) in probes.iter().enumerate() {
            let differs = a[k] != b[k];
            let inside: Vec<&diff::Region> = d.route.iter().filter(|r| holds(&r.held, p)).collect();
            assert_eq!(differs, !inside.is_empty(), "{}: {}: the stages are {:?} and {:?}, and the diff says {:?}", v.name, p.form, a[k], b[k], d.route.iter().map(|r| &r.text).collect::<Vec<_>>());
            for r in &inside {
                for c in &r.stages {
                    let at = |x: &[(String, String)]| x.iter().find(|(s, _)| *s == c.stage).map(|(_, w)| w.clone());
                    assert_eq!((at(&a[k]), at(&b[k])), (c.before.clone(), c.after.clone()), "{}: {}: the stage {}", v.name, p.form, c.stage);
                }
            }
            inside_any += usize::from(!inside.is_empty());
            tried += 1;
        }
        // every box is tried inside
        for r in &d.route {
            assert!(probes.iter().any(|p| holds(&r.held, p)), "{}: no form is tried inside {}", v.name, r.text);
        }
    }
    eprintln!("forms tried on both versions: {tried}, of which inside a box: {inside_any}; boxes: {boxes}");
    assert!(tried >= 60, "{tried}");
}

/// The diff says what changes, where, and how it reaches the tickets in progress: a box for each
/// change of the route, a section for a change the route does not show, the approvers where the
/// route stays.
#[test]
fn each_change_is_said_where_it_is() {
    let all = variants();
    let get = |n: &str| all.iter().find(|v| v.name == n).unwrap();
    let d = compared(get("expense_row"), Lang::En);
    assert_eq!(d.route.len(), 1);
    assert_eq!(d.route[0].text, "Requests where “Category” is “Expenses” and “Amount” is from 100,001 JPY to 1,000,000 JPY: the stage “precheck” (check) no longer comes.");
    assert!(d.sections.iter().all(|s| s.id != "route"), "{:?}", d.sections.iter().map(|s| &s.id).collect::<Vec<_>>());
    assert!(d.in_progress[0].contains("tickets submitted with version 1 finish as version 1 says"), "{:?}", d.in_progress);
    let d = compared(get("expense_threshold"), Lang::En);
    assert_eq!(d.route.len(), 1);
    assert!(d.route[0].text.contains("“Amount” is from 50,001 JPY to 100,000 JPY"), "{}", d.route[0].text);
    let d = compared(get("leave_condition"), Lang::En);
    assert_eq!(d.route.len(), 1, "{:?}", d.route.iter().map(|r| &r.text).collect::<Vec<_>>());
    assert!(d.route[0].text.contains("from 4 to 5"), "{}", d.route[0].text);
    let d = compared(get("contract_stage"), Lang::En);
    assert!(!d.route.is_empty());
    assert!(d.route.iter().all(|r| r.stages.iter().any(|c| c.stage == "second" && c.after.is_none())), "{:?}", d.route.iter().map(|r| &r.text).collect::<Vec<_>>());
    // the stage added: the same boxes, where it comes, answered by the people it was answered by
    let back = compared(get("contract_stage_added"), Lang::En);
    let held = |d: &Diff| {
        let mut h: Vec<String> = d.route.iter().map(|r| format!("{:?}", r.held)).collect();
        h.sort();
        h
    };
    assert_eq!(held(&back), held(&d), "{:?}", back.route.iter().map(|r| &r.text).collect::<Vec<_>>());
    assert!(back.route.iter().all(|r| r.stages.iter().any(|c| c.stage == "second" && c.before.is_none() && c.after.is_some())), "{:?}", back.route.iter().map(|r| &r.text).collect::<Vec<_>>());
    assert!(back.route.iter().all(|r| r.text.contains("the stage “second”") && r.text.ends_with(" comes.")), "{:?}", back.route.iter().map(|r| &r.text).collect::<Vec<_>>());
    // a way to choose: no box of the route, the sentence of the way, and the approvers of the stage
    let d = compared(get("account_way"), Lang::En);
    assert!(d.route.is_empty());
    let ways = d.sections.iter().find(|s| s.id == "ways").expect("the ways");
    assert_eq!(ways.changes.len(), 1, "{:?}", ways.changes);
    assert!(!d.from_data);
    assert!(d.approvers.iter().all(|g| g.changes.iter().all(|p| p.stage == "security")), "{:?}", d.approvers.iter().flat_map(|g| g.changes.iter().map(|p| &p.stage)).collect::<Vec<_>>());
    assert!(d.in_progress[0].starts_with("Both are version 1."), "{:?}", d.in_progress);
    // a deadline, a range: their sections alone
    let d = compared(get("expense_deadline"), Lang::En);
    assert!(d.route.is_empty() && d.approvers.is_empty());
    assert_eq!(d.sections.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["deadlines"]);
    let d = compared(get("expense_range"), Lang::En);
    let form = d.sections.iter().find(|s| s.id == "form").expect("the form");
    assert!(form.changes.iter().any(|c| c.item.as_deref() == Some("Items › Price — Accepts") && c.before.as_deref() == Some("from 1 JPY to 1,000,000 JPY") && c.after.as_deref() == Some("from 1 JPY to 2,000,000 JPY")), "{:?}", form.changes);
    // a person moved to another team: nothing of the request; the approvers of that person, and the
    // stages that team answers, which have one more member; from the data
    let d = compared(get("expense_person_moved"), Lang::En);
    assert!(d.route.is_empty(), "{:?}", d.route.iter().map(|r| &r.text).collect::<Vec<_>>());
    assert!(d.from_data && d.sections.is_empty());
    let moved: Vec<&diff::Person> = d.approvers.iter().flat_map(|g| &g.changes).collect();
    assert!(moved.iter().all(|p| p.id == "e1201" || p.shown.as_deref().is_some_and(|s| s.contains("accounting"))), "{:?}", moved.iter().map(|p| (&p.id, &p.stage, &p.shown)).collect::<Vec<_>>());
    let own: Vec<(&str, &str, &str)> = moved.iter().filter(|p| p.id == "e1201" && p.stage == "first").map(|p| (p.label.as_str(), p.before.as_str(), p.after.as_str())).collect();
    assert_eq!(own, [("first", "manager: e1101 (Manager of Section 1A)", "manager: e1001 (Head of Accounting)")]);
    let team: Vec<&&diff::Person> = moved.iter().filter(|p| p.id == "e0001" && p.shown.as_deref() == Some("dept_head and accounting")).collect();
    assert_eq!(team.len(), 1);
    assert_eq!((team[0].before.as_str(), team[0].after.as_str()), ("accounting: one of e1001 (Head of Accounting), e1002 (Accountant)", "accounting: one of e1001 (Head of Accounting), e1002 (Accountant), e1201 (Sales staff 1)"));
    assert!(d.in_progress.iter().any(|t| t.contains("at the next stage it starts")), "{:?}", d.in_progress);
}

/// Who answers a stage, as `simulate` gives it: each way's people, the quorum, the proxies and what
/// happens where nobody answers.
type Who = Vec<(Vec<String>, Option<usize>, Vec<(String, String)>, Option<String>)>;

fn who(st: &StageSim) -> Who {
    st.ways.iter().map(|w| (w.people.clone(), w.needed, w.proxies.clone(), w.goes.clone())).collect()
}

/// `simulate` of every requester, without a form, on a day, on one side.
fn every(side: &Side, at: Option<&str>, day: &str) -> Answer {
    let go = || {
        let run = Run::with(common::joined(), options(at));
        let o = run.file(&side.read).unwrap();
        simulate(&o, &run, &Ask { requester: None, every: true, form: None, on: On::Day(ritsu_ports::day_of(day).unwrap()) }).unwrap_or_else(|t| panic!("{}", t.en))
    };
    match side.files.clone() {
        Some(files) => ritsu_base::fs::with(files, go),
        None => go(),
    }
}

fn sim_of<'a>(a: &'a Answer, r: &Requester) -> Option<&'a ringi::simulate::Sim> {
    a.sims.iter().find(|s| s.requester == *r)
}

fn stage_of<'a>(s: &'a ringi::simulate::Sim, stage: &str, shown: &str) -> Option<&'a StageSim> {
    s.stages.iter().find(|st| st.stage == stage && st.shown == shown)
}

/// Each person `simulate` gives a stage is named in what the diff says of it: of a stage the other
/// side answers by the same ways, each person of the ways answered otherwise.
fn names_them(said: &str, st: Option<&StageSim>, other: Option<&StageSim>) -> bool {
    let Some(st) = st else { return true };
    let aligned = other.filter(|o| o.ways.len() == st.ways.len() && o.ways.iter().zip(&st.ways).all(|(x, y)| x.way == y.way));
    let (mine, theirs) = (who(st), aligned.map(who));
    st.ways.iter().enumerate().filter(|(k, _)| theirs.as_ref().is_none_or(|t| t[*k] != mine[*k])).flat_map(|(_, w)| &w.people).all(|p| said.contains(p.as_str()))
}

/// The requesters and stages the diff names, in each version of the organization, are those whose
/// stages `simulate` answers otherwise on the two sides, and each row names the people `simulate`
/// gives before and after: in a box of the route, at each stage the box changes; elsewhere, at each
/// stage both sides have.
#[test]
fn the_approvers_the_diff_names_are_the_ones_simulate_gives() {
    let mut rows = 0;
    for name in ["expense_row", "contract_stage", "account_way", "expense_person_moved"] {
        let all = variants();
        let v = all.iter().find(|v| v.name == name).unwrap();
        let d = compared(v, Lang::En);
        let (o, n) = sides(v);
        let days: &[&str] = if v.at.is_some() { &["2026-04-01"] } else { &["2026-04-01", "2027-04-01"] };
        for day in days {
            let (a, b) = (every(&o, v.at, day), every(&n, v.at, day));
            // a box of the route
            for r in &d.route {
                let mut want: Vec<(String, String)> = Vec::new();
                for s in &b.sims {
                    let Some(old) = sim_of(&a, &s.requester) else { continue };
                    for c in &r.stages {
                        let before = c.before.as_deref().and_then(|sh| stage_of(old, &c.stage, sh));
                        let after = c.after.as_deref().and_then(|sh| stage_of(s, &c.stage, sh));
                        if before.map(who) != after.map(who) {
                            want.push((s.requester.id.clone(), c.stage.clone()));
                        }
                    }
                }
                let got: Vec<&diff::Person> = r.people.iter().filter(|g| g.from == *day).flat_map(|g| &g.changes).collect();
                assert_eq!(got.iter().map(|p| (p.id.clone(), p.stage.clone())).collect::<Vec<_>>(), want, "{name}: {day}: {}", r.text);
                for p in got {
                    let c = r.stages.iter().find(|c| c.stage == p.stage).unwrap();
                    assert_eq!((p.shown.as_deref(), p.label.as_str()), (None, c.label.as_str()));
                    let (s, old) = (b.sims.iter().find(|s| s.requester.id == p.id).unwrap(), a.sims.iter().find(|s| s.requester.id == p.id).unwrap());
                    let (x, y) = (c.before.as_deref().and_then(|sh| stage_of(old, &c.stage, sh)), c.after.as_deref().and_then(|sh| stage_of(s, &c.stage, sh)));
                    assert!(names_them(&p.before, x, y), "{name}: {} before: {}", p.id, p.before);
                    assert!(names_them(&p.after, y, x), "{name}: {} after: {}", p.id, p.after);
                    assert_eq!(c.after.is_none(), p.after == "does not come", "{name}: {}", p.after);
                    assert_eq!(c.before.is_none(), p.before == "does not come", "{name}: {}", p.before);
                    rows += 1;
                }
            }
            // where the route does not change
            let mut want: Vec<(String, String, Option<String>)> = Vec::new();
            for s in &b.sims {
                let Some(old) = sim_of(&a, &s.requester) else { continue };
                for st in &s.stages {
                    let Some(was) = stage_of(old, &st.stage, &st.shown) else { continue };
                    if who(was) != who(st) {
                        want.push((s.requester.id.clone(), st.stage.clone(), Some(st.shown.clone())));
                    }
                }
            }
            let got: Vec<&diff::Person> = d.approvers.iter().filter(|g| g.from == *day).flat_map(|g| &g.changes).collect();
            assert_eq!(got.iter().map(|p| (p.id.clone(), p.stage.clone(), p.shown.clone())).collect::<Vec<_>>(), want, "{name}: {day}");
            for p in got {
                let (s, old) = (b.sims.iter().find(|s| s.requester.id == p.id).unwrap(), a.sims.iter().find(|s| s.requester.id == p.id).unwrap());
                let st = s.stages.iter().find(|st| p.stage == st.stage && p.shown.as_deref() == Some(st.shown.as_str())).unwrap();
                let was = stage_of(old, &st.stage, &st.shown);
                assert!(names_them(&p.before, was, Some(st)), "{name}: {} before: {}", p.id, p.before);
                assert!(names_them(&p.after, Some(st), was), "{name}: {} after: {}", p.id, p.after);
                rows += 1;
            }
        }
    }
    assert!(rows > 0);
    eprintln!("rows held to simulate: {rows}");
}

/// `--versions`: each requester it names is answered otherwise at the stage it says, as `simulate`
/// gives on the two days; each it does not name, alike at every stage.
#[test]
fn the_versions_name_every_requester_whose_approvers_change_and_no_other() {
    let side = Side { read: "examples/expense.ringi".to_string(), files: None, shown: "expense.ringi".to_string() };
    let (x, y) = (ritsu_ports::day_of("2026-04-01").unwrap(), ritsu_ports::day_of("2027-04-01").unwrap());
    let v = diff::versions(&side, (x, y), &common::joined(), &Options::default(), Lang::En, Lang::En).unwrap();
    assert!(v.differs());
    let (a, b) = (every(&side, None, "2026-04-01"), every(&side, None, "2027-04-01"));
    let (mut compared, mut seen) = (0, 0);
    for s in &b.sims {
        let Some(old) = sim_of(&a, &s.requester) else { continue };
        compared += 1;
        for st in &s.stages {
            let Some(was) = stage_of(old, &st.stage, &st.shown) else { continue };
            let named: Vec<&diff::Person> = v.changes.iter().filter(|p| p.id == s.requester.id && p.by == s.requester.by && p.stage == st.stage && p.shown.as_deref() == Some(st.shown.as_str())).collect();
            assert_eq!(who(was) != who(st), !named.is_empty(), "{} at {} ({})", s.requester.id, st.stage, st.shown);
            for p in named {
                assert!(names_them(&p.before, Some(was), Some(st)) && names_them(&p.after, Some(st), Some(was)), "{}: {} → {}", p.id, p.before, p.after);
            }
            seen += 1;
        }
    }
    assert_eq!(compared, v.of);
    eprintln!("stages held to simulate: {seen}, over {compared} requesters");
    assert_eq!(v.left, ["e9001 (General counsel)"]);
    // the stages that find nobody, by what happens then, counted from `simulate`: each stage, as it
    // is written, whose count moves is listed with both counts, and no other
    let then_of = |g: &str| match g {
        "open" => None,
        "skip" => Some("skipped"),
        "wait" | "cannot" => Some("waits"),
        _ => Some("elsewhere"),
    };
    let count = |ans: &Answer, stage: &str, shown: &str, then: &str| ans.sims.iter().filter(|s| s.stages.iter().any(|st| st.stage == stage && st.shown == shown && st.ways.iter().any(|w| w.goes.as_deref().and_then(then_of) == Some(then)))).count();
    let mut written: Vec<(String, String)> = a.sims.iter().chain(&b.sims).flat_map(|s| &s.stages).map(|st| (st.stage.clone(), st.shown.clone())).collect();
    written.sort();
    written.dedup();
    let mut want: Vec<(String, String, &str, usize, usize)> = Vec::new();
    for (stage, shown) in &written {
        for then in ["elsewhere", "skipped", "waits"] {
            let (x, y) = (count(&a, stage, shown, then), count(&b, stage, shown, then));
            if x != y {
                want.push((stage.clone(), shown.clone(), then, x, y));
            }
        }
    }
    let mut got: Vec<(String, String, &str, usize, usize)> = v.nobody.iter().map(|n| (n.stage.clone(), n.shown.clone(), n.then.key(), n.before, n.after)).collect();
    got.sort();
    want.sort();
    assert_eq!(got, want);
    assert!(!v.nobody.is_empty());
    // a day of the same version, a day before any, and no data
    let same = diff::versions(&side, (x, ritsu_ports::day_of("2026-05-01").unwrap()), &common::joined(), &Options::default(), Lang::En, Lang::En).unwrap();
    assert!(same.same_version() && !same.differs());
    let before = diff::versions(&side, (ritsu_ports::day_of("2025-04-01").unwrap(), y), &common::joined(), &Options::default(), Lang::En, Lang::En);
    assert!(matches!(&before, Err(diff::Refused::Said(t)) if t.en == "no version of the organization is in effect on 2025-04-01 (the first is from 2026-04-01)"), "{:?}", before.err());
}

/// What `diff` writes, held to the golden files: the text, the page in Japanese, and for one change
/// the HTML and the JSON; the same of `--versions`.
#[test]
fn what_the_diff_writes_is_held_to_its_golden_files() {
    for v in variants() {
        let en = compared(&v, Lang::En);
        ritsu_testkit::golden::golden(&format!("tests/golden/diff/{}.txt", v.name), &diff::text(&en));
        let ja = compared(&v, Lang::Ja);
        ritsu_testkit::golden::golden(&format!("tests/golden/diff/{}.ja.md", v.name), &diff::markdown(&ja));
        if v.name == "expense_row" {
            let html = diff::html(&en);
            assert!(common::html_problems(&html).is_empty(), "{:?}", common::html_problems(&html));
            ritsu_testkit::golden::golden("tests/golden/diff/expense_row.html", &html);
            ritsu_testkit::golden::golden("tests/golden/diff/expense_row.json", &format!("{}\n", diff::json(&en).pretty()));
        }
    }
    let (x, y) = (ritsu_ports::day_of("2026-04-01").unwrap(), ritsu_ports::day_of("2027-04-01").unwrap());
    for (file, name) in [("examples/expense.ringi", "expense"), ("examples/leave.ringi", "leave")] {
        let side = Side { read: file.to_string(), files: None, shown: file.strip_prefix("examples/").unwrap().to_string() };
        let en = diff::versions(&side, (x, y), &common::joined(), &Options::default(), Lang::En, Lang::En).unwrap();
        ritsu_testkit::golden::golden(&format!("tests/golden/diff/versions.{name}.txt"), &diff::versions_text(&en));
        let ja = diff::versions(&side, (x, y), &common::joined(), &Options::default(), Lang::En, Lang::Ja).unwrap();
        ritsu_testkit::golden::golden(&format!("tests/golden/diff/versions.{name}.ja.md"), &diff::versions_markdown(&ja));
        let html = diff::versions_html(&en);
        assert!(common::html_problems(&html).is_empty(), "{:?}", common::html_problems(&html));
        ritsu_testkit::golden::golden(&format!("tests/golden/diff/versions.{name}.json"), &format!("{}\n", diff::versions_json(&en).pretty()));
    }
}

/// The text, the page and the JSON say the same: each box of the route, each change of a section
/// and each requester named is in all three.
#[test]
fn the_text_the_page_and_the_json_say_the_same() {
    for v in variants() {
        let d = compared(&v, Lang::En);
        let (text, md, json) = (diff::text(&d), diff::markdown(&d), diff::json(&d).pretty());
        // the page another holds (`ritsu mod diff`): the page's lines but for the comment of its
        // head, each heading two levels down and each note a sentence
        let held = diff::markdown_held(&d);
        let body: Vec<&str> = md.lines().skip(1).collect();
        assert!(md.starts_with("<!-- ") && held.starts_with("### ") && !held.contains("<!--"), "{}:\n{held}", v.name);
        assert_eq!(held.lines().count(), body.len(), "{}:\n{held}", v.name);
        for (m, h) in body.iter().copied().zip(held.lines()) {
            let want = if m.starts_with('#') { format!("##{m}") } else { m.strip_prefix("> ").unwrap_or(m).to_string() };
            assert_eq!(h, want, "{}", v.name);
        }
        let md_text = md.replace("\\|", "|");
        for r in &d.route {
            let t = r.text.trim_end_matches('.');
            assert!(text.contains(t) && md_text.contains(t) && json.contains(t), "{}: {t}", v.name);
        }
        for s in &d.sections {
            for c in &s.changes {
                for w in [&c.item, &c.before, &c.after].into_iter().flatten() {
                    assert!(text.contains(w.as_str()) && md_text.contains(w.as_str()), "{}: {w}", v.name);
                    assert!(json.contains(&ritsu_base::json::Json::Str(w.clone()).compact()), "{}: {w}", v.name);
                }
            }
        }
        for g in d.approvers.iter().chain(d.route.iter().flat_map(|r| &r.people)) {
            for p in g.changes.iter().take(diff::NAMED) {
                assert!(text.contains(&p.named) && md_text.contains(&p.named) && json.contains(&p.id), "{}: {}", v.name, p.named);
            }
        }
    }
}

/// Two files that are not two versions of one are refused, and a version that does not pass its
/// check is refused with its diagnostics.
#[test]
fn what_cannot_be_compared_is_refused() {
    let run = |args: &[&str]| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let all: Vec<String> = std::iter::once("diff").chain(args.iter().copied()).map(str::to_string).collect();
        let code = ringi::run::run(&all, common::joined(), &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    };
    let (code, out, err) = run(&["examples/expense.ringi", "examples/shared/company.ringi"]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.contains("the two files are of different kinds (a request and an organization)"), "{err}");
    let (code, _, err) = run(&["examples/expense.ringi", "examples/leave.ringi"]);
    assert_eq!(code, 2);
    assert!(err.contains("the two files have different names (`expense` and `leave`)"), "{err}");
    let (code, _, err) = run(&["examples/contract.ringi", "examples/contract.ringi"]);
    assert_eq!(code, 2);
    assert!(err.contains("error[E504]"), "{err}");
    let (code, out, _) = run(&["examples/contract.ringi", "examples/contract.ringi", "--at", "2026-11-16"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("The two say the same and answer alike."), "{out}");
    let (code, _, err) = run(&["--versions", "2026-04-01", "2027-04-01", "examples/expense.ringi", "--at", "2026-11-16"]);
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = run(&["--versions", "2026-04-01", "2027-04-01", "examples/shared/company.ringi"]);
    assert_eq!(code, 2);
    assert!(err.contains("`--versions` compares the approvers of a request"), "{err}");
    let (code, _, _) = run(&["--versions", "2026-04-01", "examples/expense.ringi"]);
    assert_eq!(code, 2);
    let (code, _, _) = run(&["--versions", "2026-13-01", "2027-04-01", "examples/expense.ringi"]);
    assert_eq!(code, 2);
}

/// A Japanese page names the stages and the ways of a request that labels them by their labels:
/// what it says outside code spans holds none of the names the file writes them by.
#[test]
fn a_japanese_page_names_the_stages_and_the_ways_by_their_labels() {
    let names = ["precheck", "first", "second", "decide", "manager", "dept_head", "accounting", "division_head", "check", "travel_agent", "board"];
    let bare = |page: &str| -> String {
        let mut out = String::new();
        for line in page.lines().filter(|l| !l.starts_with("<!--")) {
            for (k, part) in line.split('`').enumerate() {
                if k % 2 == 0 {
                    out.push_str(part);
                }
            }
            out.push('\n');
        }
        out
    };
    let word_in = |text: &str, w: &str| text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).any(|x| x == w);
    let all = variants();
    let mut pages = Vec::new();
    for name in ["expense_row", "expense_threshold", "expense_person_moved"] {
        let v = all.iter().find(|v| v.name == name).unwrap();
        pages.push((name.to_string(), diff::markdown(&compared(v, Lang::Ja))));
    }
    let side = Side { read: "examples/expense.ringi".to_string(), files: None, shown: "expense.ringi".to_string() };
    let days = (ritsu_ports::day_of("2026-04-01").unwrap(), ritsu_ports::day_of("2027-04-01").unwrap());
    pages.push(("versions".to_string(), diff::versions_markdown(&diff::versions(&side, days, &common::joined(), &Options::default(), Lang::En, Lang::Ja).unwrap())));
    for (name, page) in &pages {
        let text = bare(page);
        let found: Vec<&&str> = names.iter().filter(|w| word_in(&text, w)).collect();
        assert!(found.is_empty(), "{name}: {found:?}\n{text}");
        assert!(["上長", "部長", "経理", "エージェントの確かめ"].iter().any(|l| text.contains(l)), "{name}");
    }
}

/// A file the requests read says how its change reaches the tickets in progress, which keep the
/// version of the request they were submitted with and no version of it: the organization and the
/// masters at the next stage a ticket starts, a shared route through the requests that use it. The
/// JSON says the same of each section (`applies_to`).
#[test]
fn each_kind_of_file_says_how_its_change_reaches_the_tickets_in_progress() {
    let variant = |name, file, edits, data| Variant { name, file, at: None, edits, data, probes: Vec::new(), grid: None, back: false };
    let cases = [
        (
            variant("company_person_moved", "examples/shared/company.ringi", vec![], Some(("examples/org/2026-04-01/members.csv", "e1201,sales1_a,staff,yes", "e1201,accounting,staff,yes"))),
            "next_stage",
            "A change of the organization reaches a ticket in progress at the next stage it starts: a stage whose approvers are decided keeps them.",
            "組織の変更は、進行中のチケットには、次に始まるステップから効きます。承認者が決まったステップは、そのまま進みます。",
        ),
        (
            variant("route_deadline", "examples/shared/manager_approval.ringi", vec![("  due in 2 business days at end of day", "  due in 3 business days at end of day")], None),
            "requests_that_use_it",
            "A shared route is part of each request that uses it, and a ticket finishes with the version of the request it was submitted with: raise the version of each request that uses it, so that the tickets in progress finish as they were submitted.",
            "共有の経路は、それを使う申請の種類の一部です。チケットは、出したときの申請の種類の版で終わります。進行中のチケットが出したときのとおりに終わるように、この経路を使う申請の種類の版を、それぞれ上げてください。",
        ),
        (
            variant("masters_owner", "examples/shared/masters.ringi", vec![], Some(("examples/masters/systems.csv", "crm,Customer system,e3101,", "crm,Customer system,e1001,"))),
            "next_stage",
            "A ticket in progress keeps the key of the row it chose: a stage whose approvers are decided keeps them, and a stage that starts after the change reads the row as the masters then say.",
            "進行中のチケットは、フォームで選んだマスタの行のキーを持ったままです。承認者が決まったステップはそのまま進み、変更のあとに始まるステップは、そのときのマスタの行を読みます。",
        ),
    ];
    for (v, applies_to, en, ja) in cases {
        let d = compared(&v, Lang::En);
        assert!(d.differs(), "{}", v.name);
        assert_eq!(d.in_progress, [en], "{}", v.name);
        assert_eq!(compared(&v, Lang::Ja).in_progress, [ja], "{}", v.name);
        // the JSON says the same of each section
        let j = diff::json(&d);
        let applies: Vec<&str> = j.get("sections").and_then(|s| s.as_arr()).unwrap_or_default().iter().map(|s| s.get("applies_to").and_then(|a| a.as_str()).unwrap_or_default()).collect();
        assert!(!applies.is_empty() && applies.iter().all(|a| *a == applies_to), "{}: {applies:?}", v.name);
    }
}

/// The JSON names a stage and its way by their ASCII names, with the names as written beside them
/// (DESIGN 3.4, 16.2): a file written in Japanese, its row changed.
#[test]
fn the_json_names_the_stages_in_ascii_beside_the_names_as_written() {
    let v = Variant {
        name: "expense_ja_row",
        file: "examples/expense.ja.ringi",
        at: None,
        edits: vec![("| 経費 | >100_000円 <=1_000_000円 | エージェント確認    |", "| 経費 | >100_000円 <=1_000_000円 |                     |"), ("request 経費(expense_ja) v1", "request 経費(expense_ja) v2")],
        data: None,
        probes: Vec::new(),
        grid: None,
        back: false,
    };
    let d = compared(&v, Lang::Ja);
    assert_eq!(d.route.len(), 1);
    let j = diff::json(&d);
    let route = j.get("route").cloned().unwrap();
    let ritsu_base::json::Json::Arr(boxes) = route else { panic!() };
    let stage = boxes[0].get("stages").cloned().unwrap();
    let ritsu_base::json::Json::Arr(stages) = stage else { panic!() };
    let get = |k: &str| stages[0].get(k).cloned().unwrap().compact();
    assert_eq!([get("stage"), get("stage_written"), get("label"), get("before"), get("before_written"), get("after")], ["\"precheck\"", "\"確かめ\"", "\"確かめ\"", "\"check\"", "\"エージェント確認\"", "null"]);
    let people = boxes[0].get("people").cloned().unwrap().compact();
    assert!(people.contains("\"stage\":\"precheck\",\"stage_written\":\"確かめ\""), "{people}");
    assert_eq!(boxes[0].get("text").cloned().unwrap().compact(), "\"「種別」が「経費」で、「合計」が100,001円から1,000,000円までの申請：ステップ「確かめ」（エージェント確認）が来なくなります。\"");
}
