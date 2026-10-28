//! `sekisho diff` (ritsu's DESIGN 17.9.4, PLAN 8.3 C.3), on the materials of `tests/diff/` (English
//! first, the Japanese versions with Japanese names): a shop whose second version lets clerks refund
//! more, forbids a suspended member, renames the managers' permit, drops an action and adds another;
//! a gate whose second version reads the second version of a rule; a gate whose second version
//! forbids what the same rule lets through.
//!
//! - Golden files of the three formats in both languages (`tests/diff/golden/`; `SEKISHO_BLESS=1`
//!   writes them).
//! - Each kind of change alone; a value no language vouches for, tried again with a concrete input,
//!   and undecided when none is found; `--budget`; what stops a diff (exit 2).
//! - Brute force: for every pair of the materials of a language (each with itself too), every
//!   request (each number at each of its values) is decided by both versions one at a time, and
//!   held to what the diff says of the combination it falls in. The number of requests is printed
//!   (`--nocapture`).

mod common;

use ritsu_base::text::Lang;
use ritsu_ports::{DaySet, Found, RuleError, Said, Values};
use sekisho::ast::Op;
use sekisho::check::Options;
use sekisho::diff::{self, Change, Diff, Side};
use sekisho::model::{FieldType, Gate, How, Source};
use sekisho::suite::Suite;
use sekisho::walk::{self, AlsoByFrame, Place};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;

const DIR: &str = "tests/diff";

const EN: [&str; 6] = ["shop.gate", "shop_v2.gate", "limits.gate", "limits_v2.gate", "held.gate", "held_v2.gate"];
const JA: [&str; 6] = ["店.gate", "店_二版.gate", "上限.gate", "上限_二版.gate", "少額.gate", "少額_二版.gate"];

fn side(name: &str, suite: &Suite) -> Side {
    diff::side(&format!("{DIR}/{name}"), suite, &Options::default()).unwrap_or_else(|t| panic!("{name}: {}", t.en))
}

fn run(old: &str, new: &str, suite: &Suite, keep: bool) -> Diff {
    let (o, n) = (side(old, suite), side(new, suite));
    diff::diff_kept(&o, &n, suite, sekisho::check::DEFAULT_BUDGET as u128, keep).unwrap_or_else(|t| panic!("{old} {new}: {}", t.en))
}

/// A rulec that cannot say what an output comes to over ranges (the port's default), whose
/// evaluator answers: what the walk counts of the band is counted in case, and is tried again with a
/// concrete input.
struct Loose(rulec::ports::Engine);

/// A rulec that can say neither: what rests on the band is undecided.
struct Silent(rulec::ports::Engine);

macro_rules! stand_in {
    ($t:ident, $eval:expr) => {
        impl ritsu_ports::Rules for $t {
            fn facts(&self, rule: &Path) -> Result<ritsu_ports::RuleFacts, Vec<Said>> {
                self.0.facts(rule)
            }
            fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<Values>)>, Vec<Said>> {
                self.0.preconditions_hold(rule, ranges, max_len)
            }
            fn output_values(&self, rule: &Path, output: &str) -> Result<Found<ritsu_ports::OutputValues>, Vec<Said>> {
                self.0.output_values(rule, output)
            }
            fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<ritsu_ports::Answer<ritsu_base::text::Text>, Vec<Said>> {
                self.0.checked_over(rule, input, days)
            }
            fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError> {
                let f: &dyn Fn(&rulec::ports::Engine, &Path, &Values) -> Result<Values, RuleError> = &$eval;
                f(&self.0, rule, inputs)
            }
            fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
                self.0.doc(rule, shown, html, lang)
            }
        }
    };
}

stand_in!(Loose, |e: &rulec::ports::Engine, rule: &Path, inputs: &Values| ritsu_ports::Rules::eval(e, rule, inputs));
stand_in!(Silent, |_: &rulec::ports::Engine, _: &Path, _: &Values| Err(RuleError::Contradiction(ritsu_base::text::Text::same("silent"))));

fn with_rules(r: Rc<dyn ritsu_ports::Rules>) -> Suite {
    let mut s = common::joined();
    s.rules = Some(r);
    s
}

/// The three formats of three diffs in each language, held to golden files.
#[test]
fn the_diffs_held_to_golden_files() {
    let suite = common::joined();
    let mut failures = Vec::new();
    for (names, lang) in [(EN, Lang::En), (JA, Lang::Ja)] {
        for (k, name) in [(0, "shop"), (2, "limits"), (4, "held")] {
            let d = run(names[k], names[k + 1], &suite, false);
            let l = if lang == Lang::Ja { "ja" } else { "en" };
            for (ext, out) in [("txt", diff::text(&d, lang)), ("md", diff::markdown(&d, lang)), ("json", diff::to_json(&d, lang).pretty() + "\n")] {
                if let Err(e) = ritsu_testkit::golden::check(Path::new(&format!("{DIR}/golden/{name}.{l}.{ext}")), &out) {
                    failures.push(e);
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn group<'a>(d: &'a Diff, action: &str, change: Change, old: &[&str], new: &[&str]) -> Option<&'a diff::Group> {
    d.actions.iter().find(|a| a.name == action)?.groups.iter().find(|g| g.change == change && g.old_policies == old && g.new_policies == new)
}

/// Each kind of change: a permission gained and lost, the deciding policies alone, what only one
/// version has; the exit codes of the command.
#[test]
fn each_kind_of_change() {
    let suite = common::joined();
    let d = run("shop.gate", "shop_v2.gate", &suite, false);
    assert_eq!(group(&d, "refund_order", Change::Gained, &[], &["clerks_refund_small_amounts"]).map(|g| g.count), Some(2));
    assert_eq!(group(&d, "refund_order", Change::Lost, &["managers_refund"], &["suspended_members_refund_nothing"]).map(|g| g.count), Some(4));
    assert_eq!(group(&d, "refund_order", Change::Reasons, &["managers_refund"], &["managers_refund_up_to_15"]).map(|g| g.count), Some(2));
    assert_eq!((d.only_old.actions.clone(), d.only_new.actions.clone()), (vec!["cancel_order".to_string()], vec!["approve_refund".to_string()]));
    assert_eq!((d.only_new.roles.clone(), d.only_old.attributes.clone(), d.only_new.attributes.clone()), (vec!["auditor".to_string()], vec!["User.seniority".to_string()], vec!["User.region".to_string()]));
    assert!(d.differs());
    // the same file: nothing changes
    let d = run("shop.gate", "shop.gate", &suite, false);
    assert!(!d.differs() && d.actions.iter().all(|a| a.groups.is_empty() && a.unknown == 0));
    // a rule read in its second version: what the band changes to is a change of the answers
    let d = run("limits.gate", "limits_v2.gate", &suite, false);
    assert_eq!(group(&d, "refund_order", Change::Gained, &[], &["clerks_refund_small_amounts"]).map(|g| g.count), Some(1));
    assert!(d.actions[0].groups.iter().all(|g| g.change != Change::Lost), "no amount the new band calls large was small before");
    // only the deciding policies change: exit 0
    let (o, n) = (side("held_v2.gate", &suite), side("held_v2.gate", &suite));
    let d = diff::diff(&o, &n, &suite, 1_000).unwrap();
    assert!(!d.differs());
}

/// A value no language vouches for: tried again with a concrete input on each version, it changes
/// as the exact walk says; when no input can be had, it is undecided, counted apart and never as
/// the same, and nothing is said of the other combinations.
#[test]
fn a_value_no_language_vouches_for() {
    let exact = run("held.gate", "held_v2.gate", &common::joined(), false);
    let loose = run("held.gate", "held_v2.gate", &with_rules(Rc::new(Loose(rulec::ports::Engine::default()))), false);
    let count = |d: &Diff| d.actions[0].groups.iter().map(|g| (g.change, g.count)).collect::<Vec<_>>();
    assert_eq!(count(&loose).iter().filter(|(c, _)| *c == Change::Lost).count(), count(&exact).iter().filter(|(c, _)| *c == Change::Lost).count());
    assert_eq!(loose.actions[0].unknown, 0);
    let silent = run("held.gate", "held_v2.gate", &with_rules(Rc::new(Silent(rulec::ports::Engine::default()))), false);
    assert!(silent.actions[0].unknown > 0, "{}", diff::text(&silent, Lang::En));
    assert!(silent.actions[0].groups.iter().all(|g| g.change != Change::Lost), "a loss that rests on the band is not said");
    let t = diff::text(&silent, Lang::En);
    assert!(t.contains("cannot be decided") && t.ends_with("some combinations cannot be decided, so nothing is said of the others.\n"), "{t}");
    let t = diff::text(&silent, Lang::Ja);
    assert!(t.contains("決められない") && t.ends_with("決められない組み合わせがあるので、ほかの組み合わせが同じ決定になるとは言いません。\n"), "{t}");
    let j = diff::to_json(&silent, Lang::En);
    assert_eq!(j.get("actions").and_then(|a| a.as_arr()).and_then(|a| a[0].get("same_elsewhere")).and_then(|x| x.as_bool()), Some(false));
}

fn sekisho(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_sekisho")).args(args).env_remove("SEKISHO_LANG").env_remove("RITSU_LANG").output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The command: its exit codes, `--budget`, and what stops it (exit 2).
#[test]
fn the_command_and_what_stops_it() {
    let (code, out, err) = sekisho(&["diff", "tests/diff/shop.gate", "tests/diff/shop_v2.gate", "--format", "json"]);
    assert_eq!((code, err.as_str()), (1, ""), "{out}");
    assert!(out.starts_with("{\n  \"sekisho\": "), "{out}");
    let (code, _, err) = sekisho(&["diff", "tests/diff/shop.gate", "tests/diff/shop.gate"]);
    assert_eq!((code, err.as_str()), (0, ""));
    let (code, _, err) = sekisho(&["diff", "tests/diff/shop.gate", "tests/diff/shop_v2.gate", "--budget", "10"]);
    assert_eq!((code, err.as_str()), (2, "error: the walk of `refund_order` over both versions has 64 combinations, over the budget of 10; give a larger `--budget`\n"));
    let (code, _, err) = sekisho(&["diff", "tests/diff/店.gate", "tests/diff/店_二版.gate", "--budget", "10", "--lang", "ja"]);
    assert_eq!((code, err.as_str()), (2, "エラー: `返金する` を二つの版で合わせてたどる組み合わせは 64 通りで、上限 10 を超えます。`--budget` を大きくしてください\n"));
    // a gate that reads a rule, with this sekisho, which reads none
    let (code, _, err) = sekisho(&["diff", "tests/diff/held.gate", "tests/diff/held_v2.gate"]);
    assert_eq!((code, err.as_str()), (2, "error: `tests/diff/held.gate` reads a file of a language this sekisho does not read; run `ritsu sekisho diff tests/diff/held.gate tests/diff/held_v2.gate`\n"));
    let (code, _, err) = sekisho(&["diff", "tests/diff/shop.gate"]);
    assert_eq!((code, err.as_str()), (2, "error: `sekisho diff` takes two sides, the old and the new\n"));
    let (code, _, err) = sekisho(&["diff", "tests/diff/shop.gate", "tests/diff/none.gate"]);
    assert_eq!(code, 2);
    assert!(err.starts_with("error: cannot read `tests/diff/none.gate`"), "{err}");
    let (code, _, err) = sekisho(&["diff", "tests/diff/shop.gate", "tests/none.gate@HEAD"]);
    assert_eq!(code, 2, "{err}");
}

// ── The brute force ─────────────────────────────────────────────────────

/// What a request comes to over both versions: how it changes and the deciding policies of each, or
/// None for the same.
type Class = Option<(Change, Vec<String>, Vec<String>)>;

/// The field of an attribute or an input of an action's frame, by name, in a gate.
fn field<'g>(g: &'g Gate, action: &str, frame: &(String, String), p: &Place) -> Option<&'g sekisho::model::Field> {
    match p {
        Place::Attr(o, n) => {
            let ty = match o {
                sekisho::model::Owner::Principal => &frame.0,
                sekisho::model::Owner::Resource => &frame.1,
            };
            g.types.iter().find(|t| t.named.name == *ty)?.attrs.iter().find(|f| f.named.name == *n)
        }
        Place::Input(n) => g.actions.iter().find(|a| a.named.name == action)?.inputs.iter().find(|f| f.named.name == *n),
    }
}

/// What both versions read of an action, each number cut at each of its values, with what the rules
/// are given: every combination of the walk is one request.
fn every_request(sides: [&Side; 2], action: &str) -> AlsoByFrame {
    let mut all = AlsoByFrame::new();
    for s in sides {
        let c = s.outcome.walked.as_ref().unwrap();
        let Some(ai) = c.gate.actions.iter().position(|a| a.named.name == action) else { continue };
        let reads = walk::reads(&c.gate, ai, &c.known);
        for (frame, also) in reads {
            let into = all.entry(frame).or_default();
            into.roles.extend(also.roles);
            into.workflow |= also.workflow;
            for (p, cmps) in also.places {
                match into.places.iter_mut().find(|(q, _)| *q == p) {
                    Some((_, c)) => c.extend(cmps),
                    None => into.places.push((p, cmps)),
                }
            }
            for (t, (terms, members)) in also.terms {
                let e = into.terms.entry(t).or_default();
                for x in terms {
                    if !e.0.contains(&x) {
                        e.0.push(x);
                    }
                }
                e.1.extend(members);
            }
        }
        // what the rules are given
        let a = &c.gate.actions[ai];
        for frame in all.keys().cloned().collect::<Vec<_>>() {
            for cv in &a.computed {
                if let How::Rule { args, .. } = &cv.how {
                    for (_, src) in args {
                        let p = match src {
                            Source::Attr(o, n) => Place::Attr(*o, n.clone()),
                            Source::Input(n) => Place::Input(n.clone()),
                            _ => continue,
                        };
                        let into = all.get_mut(&frame).unwrap();
                        if !into.places.iter().any(|(q, _)| *q == p) {
                            into.places.push((p, Vec::new()));
                        }
                    }
                }
            }
        }
    }
    // each number at each of its values
    for (frame, also) in all.iter_mut() {
        for (p, cmps) in also.places.iter_mut() {
            for s in sides {
                let g = &s.outcome.walked.as_ref().unwrap().gate;
                if let Some(FieldType::Num { lo, hi, .. }) = field(g, action, frame, p).map(|f| &f.ty) {
                    cmps.extend((*lo..=*hi).map(|k| (Op::Is, k)));
                }
            }
        }
    }
    all
}

/// Every request of each action both versions have, decided by each: how it changes.
fn brute_force(o: &Side, n: &Side, suite: &Suite) -> BTreeMap<String, Vec<(BTreeMap<String, String>, Class)>> {
    let mut out = BTreeMap::new();
    let (go, gn) = (&o.outcome.walked.as_ref().unwrap().gate, &n.outcome.walked.as_ref().unwrap().gate);
    for a in &gn.actions {
        if !go.actions.iter().any(|x| x.named.name == a.named.name) {
            continue;
        }
        let also = every_request([o, n], &a.named.name);
        let mut olds = Vec::new();
        diff::each(o, "old ", &a.named.name, &also, suite, u128::MAX, |v, allow, names| olds.push((v.into_iter().collect::<BTreeMap<_, _>>(), allow, names))).unwrap();
        let mut news = Vec::new();
        diff::each(n, "new ", &a.named.name, &also, suite, u128::MAX, |v, allow, names| news.push((v.into_iter().collect::<BTreeMap<_, _>>(), allow, names))).unwrap();
        // the requests both versions take: joined on what both read
        let common = |x: &BTreeMap<String, String>, y: &BTreeMap<String, String>| x.iter().all(|(k, v)| k.starts_with("old ") || !y.contains_key(k) || y[k] == *v);
        let mut reqs = Vec::new();
        for (vo, ao, po) in &olds {
            for (vn, an, pn) in &news {
                if !common(vo, vn) {
                    continue;
                }
                let mut v = vo.clone();
                v.extend(vn.clone());
                let class = if ao != an {
                    Some((if *an { Change::Gained } else { Change::Lost }, po.clone(), pn.clone()))
                } else if po != pn {
                    Some((Change::Reasons, po.clone(), pn.clone()))
                } else {
                    None
                };
                reqs.push((v, class));
            }
        }
        out.insert(a.named.name.clone(), reqs);
    }
    out
}

/// Whether a request is in a combination of the diff: each value the combination names is the
/// request's, or a cell the request's number is in. A value both versions compute alike is named
/// once in the diff, and once for each version by the brute force.
fn within(req: &BTreeMap<String, String>, combo: &[(String, String)]) -> bool {
    let cell = |s: &str| -> Option<(i128, i128)> { s.split_once("..").and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))) };
    combo.iter().all(|(k, v)| {
        let keys: Vec<String> = if k.starts_with("value ") { vec![format!("old {k}"), format!("new {k}")] } else { vec![k.clone()] };
        keys.iter().all(|k| match (req.get(k), cell(v)) {
            (None, _) => true,
            (Some(r), Some((lo, hi))) => cell(r).is_some_and(|(x, y)| x == y && lo <= x && x <= hi),
            (Some(r), None) => r == v,
        })
    })
}

/// Every pair of the materials of a language (each with itself too), with every language's own
/// rulec, with one that cannot say what the band comes to over ranges, and with one that cannot
/// evaluate it either: every combination the diff says changes holds requests that all change so,
/// every request that changes is in a combination the diff says changes so (or undecided), and every
/// combination the diff says changes for certain holds a request.
#[test]
fn the_diffs_are_what_the_brute_force_finds() {
    let real = common::joined();
    let suites: [(&str, Suite); 3] = [
        ("exact", real.clone()),
        ("loose", with_rules(Rc::new(Loose(rulec::ports::Engine::default())))),
        ("silent", with_rules(Rc::new(Silent(rulec::ports::Engine::default())))),
    ];
    let (mut pairs, mut requests, mut combos) = (0u64, 0u64, 0u64);
    for names in [EN, JA] {
        for a in names {
            for b in names {
                let truth = brute_force(&side(a, &real), &side(b, &real), &real);
                for (how, suite) in &suites {
                    let d = run(a, b, suite, true);
                    pairs += 1;
                    for ad in &d.actions {
                        let reqs = &truth[&ad.name];
                        combos += ad.combinations as u64;
                        if *how == "exact" {
                            requests += reqs.len() as u64;
                        }
                        for m in &ad.detail {
                            let inside: Vec<&(BTreeMap<String, String>, Class)> = reqs.iter().filter(|(r, _)| within(r, &m.values)).collect();
                            if let Some(c) = &m.change {
                                assert!(!inside.is_empty(), "{a} → {b} ({how}): {:?} holds no request", m.values);
                                for (r, rc) in &inside {
                                    assert_eq!(rc.as_ref(), Some(c), "{a} → {b} ({how}): {r:?} in {:?}", m.values);
                                }
                            }
                        }
                        for (r, rc) in reqs {
                            let Some(c) = rc else { continue };
                            let said = ad.detail.iter().any(|m| within(r, &m.values) && (m.change.is_none() || m.change.as_ref() == Some(c)));
                            assert!(said, "{a} → {b} ({how}): {r:?} changes {c:?}, which the diff does not say");
                        }
                        if *how != "silent" {
                            assert_eq!(ad.unknown, 0, "{a} → {b} ({how})");
                        }
                    }
                }
            }
        }
    }
    eprintln!("sekisho diff, brute force: {pairs} diffs ({} pairs, each with three rulecs), {requests} requests decided by both versions, {combos} combinations walked", pairs / 3);
}

/// A side at a git revision reads the rule at that revision: a commit that changes only the rule
/// changes the answers.
#[test]
fn a_side_at_a_git_revision() {
    if Command::new("git").arg("--version").output().is_err() {
        ritsu_testkit::skip("git is not on the PATH; a side at a revision is read with it");
        return;
    }
    let t = ritsu_testkit::TempDir::new("sekisho-diff-git");
    let d = t.path();
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .arg("-C")
            .arg(d)
            .args(["-c", "user.name=sekisho-test", "-c", "user.email=sekisho-test@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    std::fs::copy(format!("{DIR}/limits.gate"), d.join("limits.gate")).unwrap();
    std::fs::copy(format!("{DIR}/band.rule"), d.join("band.rule")).unwrap();
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "limits"]);
    // the working tree's rule is the second version, under the first one's name
    std::fs::copy(format!("{DIR}/band_v2.rule"), d.join("band.rule")).unwrap();
    // the repository is the one the current directory is in, as the command finds it; this test
    // says where that is without moving the process (the other tests read paths from the crate)
    let suite = common::joined();
    let at = |spelled: String| ritsu_base::fs::with(Rc::new(At(d.to_path_buf())), || diff::side(&spelled, &suite, &Options::default())).map_err(|t| t.en);
    let got = (|| -> Result<Diff, String> {
        let o = at(format!("{}@HEAD", d.join("limits.gate").display()))?;
        let n = at(d.join("limits.gate").display().to_string())?;
        diff::diff(&o, &n, &suite, 1_000_000).map_err(|t| t.en)
    })();
    let d = got.unwrap();
    assert_eq!(group(&d, "refund_order", Change::Gained, &[], &["clerks_refund_small_amounts"]).map(|g| g.count), Some(1), "{}", diff::text(&d, Lang::En));
}

/// The disk, its current directory said apart from the process's.
struct At(std::path::PathBuf);

impl At {
    fn abs(&self, p: &Path) -> std::path::PathBuf {
        if p.is_absolute() { p.to_path_buf() } else { self.0.join(p) }
    }
}

impl ritsu_base::fs::Files for At {
    fn read(&self, p: &Path) -> std::io::Result<Vec<u8>> {
        ritsu_base::fs::Disk.read(&self.abs(p))
    }
    fn metadata(&self, p: &Path) -> std::io::Result<ritsu_base::fs::Meta> {
        ritsu_base::fs::Disk.metadata(&self.abs(p))
    }
    fn read_dir(&self, p: &Path) -> std::io::Result<Vec<(std::ffi::OsString, ritsu_base::fs::Kind)>> {
        ritsu_base::fs::Disk.read_dir(&self.abs(p))
    }
    fn write(&self, p: &Path, bytes: &[u8]) -> std::io::Result<()> {
        ritsu_base::fs::Disk.write(&self.abs(p), bytes)
    }
    fn create_dir_all(&self, p: &Path) -> std::io::Result<()> {
        ritsu_base::fs::Disk.create_dir_all(&self.abs(p))
    }
    fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
        Ok(self.0.clone())
    }
    fn canonicalize(&self, p: &Path) -> std::io::Result<std::path::PathBuf> {
        ritsu_base::fs::Disk.canonicalize(&self.abs(p))
    }
}
