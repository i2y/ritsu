//! What the reads of one collection can say together, held to every small collection (§15.281).
//!
//! `rulec check` reads the definitions of `any`, `all`, `count`, `sum`, `max` and `min` together
//! now, and what it finds changes what it says. A wrong answer either stops a right rule at E123
//! or lets a wrong one through, so the answers are not taken on trust here. Each scenario lists
//! what a field of an element can hold, writes out every small request there is, and works each
//! read out with an evaluator that shares nothing with `walks.rs`. Then every question the check
//! could ask is put to it, and the answer is held to what the requests really come to:
//!
//! - "does not occur" has to be true of every request written out, and a `never` line the check
//!   takes (§15.282) has to be one, while a line it stops has to be given by a request;
//! - "occurs" comes with a request, which is worked out here and has to give the combination and
//!   pass the contract;
//! - where the contract itself stops the collection at the size written out, the answer has to
//!   be exact as well, so a check that goes quiet shows up.
//!
//! The evaluator is held to the generated Python in turn, on the same requests, so it cannot
//! drift from what a request comes to at run time. E123, the note on E101 and W133 are then held
//! to the same requests through `check` itself.

use ritsu_testkit::{Need, TempDir, ready};
use rulec::diag::{Diag, FixKind};
use rulec::fourier::{Ineq, Lin};
use rulec::i18n::{self, Lang};
use rulec::num::Rat;
use rulec::projection::walks::{Ask, Example, Possible, Walks};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

// ── What a request holds ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum V {
    W(String),
    N(f64),
    T(bool),
}

type El = BTreeMap<String, V>;

#[derive(Clone, Debug, Default)]
struct Req {
    top: El,
    walks: BTreeMap<String, Vec<El>>,
}

fn word<'a>(e: &'a El, f: &str) -> &'a str {
    match e.get(f) {
        Some(V::W(w)) => w,
        x => panic!("`{f}` is not a word here: {x:?}"),
    }
}

fn num(e: &El, f: &str) -> f64 {
    match e.get(f) {
        Some(V::N(n)) => *n,
        x => panic!("`{f}` is not a number here: {x:?}"),
    }
}

fn yes(e: &El, f: &str) -> bool {
    match e.get(f) {
        Some(V::T(b)) => *b,
        x => panic!("`{f}` is not a truth value here: {x:?}"),
    }
}

fn walk<'r>(q: &'r Req, at: &str) -> &'r [El] {
    q.walks.get(at).map_or(&[], |v| v.as_slice())
}

fn words(ws: &[&str]) -> Vec<V> {
    ws.iter().map(|w| V::W((*w).to_string())).collect()
}

fn nums(ns: &[f64]) -> Vec<V> {
    ns.iter().map(|n| V::N(*n)).collect()
}

fn truths() -> Vec<V> {
    vec![V::T(false), V::T(true)]
}

// ── The reads, written twice: as the rule writes them, and as this file works them out ─────

/// A number as a rule writes it.
fn lit(v: f64) -> String {
    if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") }
}

/// The evaluator's numbers are compared in thousandths, which holds every value used here.
fn key(v: f64) -> i64 {
    (v * 1000.0).round() as i64
}

fn rat(v: f64) -> Rat {
    Rat::new(key(v) as i128, 1000)
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Truth,
    Number,
    Word,
}

struct R {
    name: String,
    alias: Option<&'static str>,
    sort: Sort,
    /// For a word: the values, and each evaluates to its place among them.
    values: Vec<&'static str>,
    ty: String,
    unit: &'static str,
    range: Option<(f64, f64)>,
    empty: Option<f64>,
    /// What follows the type: `from count order.lines where a = b`.
    from: String,
    /// A summary over a sequence (the whole line is its own), or an input.
    summary: Option<String>,
    /// What the read does: a `sum` and a `max` or `min` of one field asked together is the one
    /// question that is not linear, and the check may be unable to say.
    does: &'static str,
    eval: Box<dyn Fn(&Req) -> f64>,
}

impl R {
    fn new(name: &str, sort: Sort, from: String, eval: Box<dyn Fn(&Req) -> f64>) -> R {
        R {
            name: name.to_string(),
            alias: None,
            sort,
            values: Vec::new(),
            ty: if sort == Sort::Truth { "bool".to_string() } else { "number".to_string() },
            unit: "",
            range: None,
            empty: None,
            from,
            summary: None,
            does: "",
            eval,
        }
    }

    fn doing(mut self, what: &'static str) -> R {
        self.does = what;
        self
    }

    fn alias(mut self, a: &'static str) -> R {
        self.alias = Some(a);
        self
    }

    /// The same read in another type: `mass[g]` with the unit its literals carry.
    fn typed(mut self, ty: &str, unit: &'static str) -> R {
        self.ty = ty.to_string();
        self.unit = unit;
        self
    }

    fn head(&self) -> String {
        match self.alias {
            Some(a) => format!("{}({a})", self.name),
            None => self.name.clone(),
        }
    }

    /// The input's line in `inputs`, or the summary's line of its own.
    fn line(&self) -> String {
        let range = self.range.map_or(String::new(), |(lo, hi)| format!("  range >={}{u} <={}{u}", lit(lo), lit(hi), u = self.unit));
        let empty = self.empty.map_or(String::new(), |e| format!(" empty {}{}", lit(e), self.unit));
        match &self.summary {
            Some(s) => format!("{s}{empty}{range}"),
            None => format!("  {} : {}{range}  {}{empty}", self.head(), self.ty, self.from),
        }
    }

    fn inside(&self, v: f64) -> bool {
        self.range.is_none_or(|(lo, hi)| lo <= v && v <= hi)
    }

    fn cell(&self, op: &str, v: f64) -> String {
        format!("{op}{}{}", lit(v), self.unit)
    }
}

fn collection_key(at: &str) -> String {
    match at.split_once('.') {
        Some((_, k)) => k.to_string(),
        None => at.to_string(),
    }
}

fn with_where(w: &str) -> String {
    if w.is_empty() { String::new() } else { format!(" where {w}") }
}

fn any(name: &str, at: &str, w: &str, p: fn(&El) -> bool) -> R {
    let k = collection_key(at);
    R::new(name, Sort::Truth, format!("from any {at} where {w}"), Box::new(move |q| f64::from(walk(q, &k).iter().any(p))))
}

fn all(name: &str, at: &str, w: &str, p: fn(&El) -> bool) -> R {
    let k = collection_key(at);
    R::new(name, Sort::Truth, format!("from all {at} where {w}"), Box::new(move |q| f64::from(walk(q, &k).iter().all(p))))
}

fn count(name: &str, at: &str, w: &str, p: fn(&El) -> bool, range: (f64, f64)) -> R {
    let k = collection_key(at);
    let mut r = R::new(name, Sort::Number, format!("from count {at}{}", with_where(w)), Box::new(move |q| walk(q, &k).iter().filter(|e| p(e)).count() as f64));
    r.range = Some(range);
    r
}

fn sum(name: &str, at: &str, of: &'static str, w: &str, p: fn(&El) -> bool, range: (f64, f64)) -> R {
    let k = collection_key(at);
    let mut r = R::new(name, Sort::Number, format!("from sum {at} of {of}{}", with_where(w)), Box::new(move |q| walk(q, &k).iter().filter(|e| p(e)).map(|e| num(e, of)).fold(0.0, |a, b| a + b))).doing("sum");
    r.range = Some(range);
    r
}

fn max(name: &str, at: &str, of: &'static str, w: &str, p: fn(&El) -> bool, empty: f64, range: (f64, f64)) -> R {
    let k = collection_key(at);
    let mut r = R::new(
        name,
        Sort::Number,
        format!("from max {at} of {of}{}", with_where(w)),
        Box::new(move |q| walk(q, &k).iter().filter(|e| p(e)).map(|e| num(e, of)).reduce(f64::max).unwrap_or(empty)),
    )
    .doing("extreme");
    r.empty = Some(empty);
    r.range = Some(range);
    r
}

fn min(name: &str, at: &str, of: &'static str, w: &str, p: fn(&El) -> bool, empty: f64, range: (f64, f64)) -> R {
    let k = collection_key(at);
    let mut r = R::new(
        name,
        Sort::Number,
        format!("from min {at} of {of}{}", with_where(w)),
        Box::new(move |q| walk(q, &k).iter().filter(|e| p(e)).map(|e| num(e, of)).reduce(f64::min).unwrap_or(empty)),
    )
    .doing("extreme");
    r.empty = Some(empty);
    r.range = Some(range);
    r
}

fn plain_number(name: &str, at: &str, range: (f64, f64)) -> R {
    let k = collection_key(at);
    let mut r = R::new(name, Sort::Number, format!("from {at}"), Box::new(move |q| num(&q.top, &k)));
    r.range = Some(range);
    r
}

fn plain_truth(name: &str, at: &str) -> R {
    let k = collection_key(at);
    R::new(name, Sort::Truth, format!("from {at}"), Box::new(move |q| f64::from(yes(&q.top, &k))))
}

fn plain_word(name: &str, at: &str, ty: &str, values: &[&'static str]) -> R {
    let k = collection_key(at);
    let vs: Vec<&'static str> = values.to_vec();
    let held = vs.clone();
    let mut r = R::new(name, Sort::Word, format!("from {at}"), Box::new(move |q| held.iter().position(|v| *v == word(&q.top, &k)).expect("a value the scenario lists") as f64));
    r.values = vs;
    r.ty = ty.to_string();
    r
}

fn over(name: &str, line: String, eval: Box<dyn Fn(&Req) -> f64>) -> R {
    let mut r = R::new(name, Sort::Number, String::new(), eval);
    r.summary = Some(line);
    r
}

fn over_count(name: &str, seq: &'static str, w: &str, p: fn(&El) -> bool, range: (f64, f64)) -> R {
    let mut r = over(name, format!("count {name} over {seq}{}", with_where(w)), Box::new(move |q| walk(q, seq).iter().filter(|e| p(e)).count() as f64));
    r.range = Some(range);
    r
}

fn over_sum(name: &str, seq: &'static str, of: &'static str, range: (f64, f64)) -> R {
    let mut r = over(name, format!("sum {name} over {seq} of {of}"), Box::new(move |q| walk(q, seq).iter().map(|e| num(e, of)).fold(0.0, |a, b| a + b))).doing("sum");
    r.range = Some(range);
    r
}

fn over_max(name: &str, seq: &'static str, of: &'static str, w: &str, p: fn(&El) -> bool, empty: f64) -> R {
    let mut r = over(
        name,
        format!("max {name} over {seq} of {of}{}", with_where(w)),
        Box::new(move |q| walk(q, seq).iter().filter(|e| p(e)).map(|e| num(e, of)).reduce(f64::max).unwrap_or(empty)),
    )
    .doing("extreme");
    r.empty = Some(empty);
    r
}

fn over_min(name: &str, seq: &'static str, of: &'static str, w: &str, p: fn(&El) -> bool, empty: f64) -> R {
    let mut r = over(
        name,
        format!("min {name} over {seq} of {of}{}", with_where(w)),
        Box::new(move |q| walk(q, seq).iter().filter(|e| p(e)).map(|e| num(e, of)).reduce(f64::min).unwrap_or(empty)),
    )
    .doing("extreme");
    r.empty = Some(empty);
    r
}

// ── A scenario ─────────────────────────────────────────────────────────────────────────────

/// A field of an element, or of the object itself, and what it can hold. `closed` when these are
/// all of its values, so an example can be held to them.
struct F {
    name: &'static str,
    values: Vec<V>,
    closed: bool,
}

fn closed(name: &'static str, values: Vec<V>) -> F {
    F { name, values, closed: true }
}

/// Values picked to stand for the classes a read tells apart: the contract lets through others.
fn sample(name: &'static str, values: Vec<V>) -> F {
    F { name, values, closed: false }
}

struct Coll {
    key: &'static str,
    fields: Vec<F>,
    least: usize,
    most: usize,
    /// Whether the contract stops the collection at `most`.
    capped: bool,
}

#[derive(Default)]
struct S {
    tag: &'static str,
    /// The file beside the rule, when there is one.
    file: Option<(&'static str, String)>,
    /// What the rule declares before its inputs: the `shape`, an `enum`, the `elements`.
    head: String,
    /// The root the paths start from.
    root: &'static str,
    colls: Vec<Coll>,
    plain: Vec<F>,
    /// A condition the contract places across the fields, as this file works it out.
    valid: Option<fn(&Req) -> bool>,
    reads: Vec<R>,
    /// Whether the check reads everything here and the requests are all there are, so its answers
    /// have to be exact and not only true.
    exact: bool,
    /// The values protojson leaves out.
    left_out: Vec<V>,
    /// Questions over more than two reads are put at every `triples`-th place.
    triples: usize,
    /// The sequence comes from the contract and is summed inside the rule, so there is no one
    /// place the generated code hands the values over.
    no_read_back: bool,
}

impl S {
    /// Every multiset of elements of one collection.
    fn multisets(c: &Coll) -> Vec<Vec<El>> {
        let mut kinds: Vec<El> = vec![El::new()];
        for f in &c.fields {
            let mut next = Vec::with_capacity(kinds.len() * f.values.len());
            for k in &kinds {
                for v in &f.values {
                    let mut e = k.clone();
                    e.insert(f.name.to_string(), v.clone());
                    next.push(e);
                }
            }
            kinds = next;
        }
        fn go(kinds: &[El], from: usize, left: usize, have: &mut Vec<El>, out: &mut Vec<Vec<El>>) {
            out.push(have.clone());
            if left == 0 {
                return;
            }
            for i in from..kinds.len() {
                have.push(kinds[i].clone());
                go(kinds, i, left - 1, have, out);
                have.pop();
            }
        }
        let mut out = Vec::new();
        go(&kinds, 0, c.most, &mut Vec::new(), &mut out);
        out.retain(|m| m.len() >= c.least);
        out
    }

    /// Every request written out, the ones the contract lets through.
    fn requests(&self) -> Vec<Req> {
        let mut out = vec![Req::default()];
        for f in &self.plain {
            let mut next = Vec::with_capacity(out.len() * f.values.len());
            for q in &out {
                for v in &f.values {
                    let mut n = q.clone();
                    n.top.insert(f.name.to_string(), v.clone());
                    next.push(n);
                }
            }
            out = next;
        }
        for c in &self.colls {
            let ms = S::multisets(c);
            let mut next = Vec::with_capacity(out.len() * ms.len());
            for q in &out {
                for m in &ms {
                    let mut n = q.clone();
                    n.walks.insert(c.key.to_string(), m.clone());
                    next.push(n);
                }
            }
            out = next;
        }
        if let Some(v) = self.valid {
            out.retain(v);
        }
        out
    }

    /// A rule that reads `which`, with a `constraint` if one is given, and `table` as its table.
    fn rule(&self, which: &[usize], constraint: Option<&str>, table: &str) -> String {
        let mut s = format!("rule t(t) v1\n\n{}\n\n", self.head);
        let reads: Vec<&R> = which.iter().map(|i| &self.reads[*i]).collect();
        let inputs: Vec<String> = reads.iter().filter(|r| r.summary.is_none()).map(|r| r.line()).collect();
        if !inputs.is_empty() {
            s.push_str("inputs\n");
            s.push_str(&inputs.join("\n"));
            s.push_str("\n\n");
        }
        for r in reads.iter().filter(|r| r.summary.is_some()) {
            s.push_str(&r.line());
            s.push('\n');
        }
        if let Some(k) = constraint {
            s.push_str(&format!("\n{k}\n"));
        }
        s.push_str("\noutputs\n  x(x) : number  round down(1)\n\n");
        s.push_str(table);
        s
    }

    fn catch_all(&self, i: usize) -> String {
        format!("table k\npolicy first\n| {} | -> x : number |\n| - | 1 |\n", self.reads[i].name)
    }

    fn path(&self, tmp: &Path) -> String {
        tmp.join("t.rule").to_string_lossy().into_owned()
    }

    /// The numbers a `constraint` can name: a summary is not one of them.
    fn numbers(&self) -> Vec<usize> {
        (0..self.reads.len()).filter(|i| self.reads[*i].sort == Sort::Number && self.reads[*i].summary.is_none()).collect()
    }

    /// The values worked out on every request, each read's `range` holding on `within`.
    fn values(&self, reqs: &[Req], within: &[usize]) -> Vec<Vec<f64>> {
        reqs.iter()
            .map(|q| {
                let mut got = vec![f64::NAN; self.reads.len()];
                for i in within {
                    got[*i] = (self.reads[*i].eval)(q);
                }
                got
            })
            .filter(|v| within.iter().all(|i| self.reads[*i].inside(v[*i])))
            .collect()
    }

    /// The request an example shows, worked out on what it shows and held to the contract.
    fn request_of(&self, ex: &Example, tell: &str) -> Req {
        let text = |t: &str| -> V {
            if let Some(w) = t.strip_prefix('"') {
                return V::W(w.trim_end_matches('"').to_string());
            }
            match t {
                "true" => V::T(true),
                "false" => V::T(false),
                _ => match t.split_once('/') {
                    Some((a, b)) => V::N(a.parse::<f64>().unwrap() / b.parse::<f64>().unwrap()),
                    None => V::N(t.parse().unwrap_or_else(|_| panic!("{tell}: `{t}` is no value"))),
                },
            }
        };
        let strip = |p: &str| -> String {
            match p.strip_prefix(self.root).and_then(|r| r.strip_prefix('.')) {
                Some(r) => r.to_string(),
                None => p.to_string(),
            }
        };
        let mut q = Req::default();
        for (p, t) in &ex.plain {
            let k = strip(p);
            let v = text(t);
            if let Some(f) = self.plain.iter().find(|f| f.name == k) {
                assert!(!f.closed || f.values.contains(&v), "{tell}: the example's `{k}` = {t} is not a value the contract lets through");
            }
            q.top.insert(k, v);
        }
        for w in &ex.walks {
            let k = strip(&w.path);
            let c = self.colls.iter().find(|c| c.key == k).unwrap_or_else(|| panic!("{tell}: the example shows `{k}`, which is no collection"));
            let mut es: Vec<El> = Vec::new();
            for (n, fields) in &w.elements {
                let mut e = El::new();
                for (name, t) in fields {
                    let v = text(t);
                    let f = c.fields.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("{tell}: the example shows `{name}`, which is no field"));
                    assert!(!f.closed || f.values.contains(&v), "{tell}: the example's `{name}` = {t} is not a value the contract lets through");
                    e.insert(name.clone(), v);
                }
                for _ in 0..*n {
                    es.push(e.clone());
                }
            }
            assert!(es.len() >= c.least, "{tell}: the example's `{k}` has {} elements, under the contract's {}", es.len(), c.least);
            assert!(!c.capped || es.len() <= c.most, "{tell}: the example's `{k}` has {} elements, over the contract's {}", es.len(), c.most);
            q.walks.insert(k, es);
        }
        q
    }
}

// ── The first thing held: `Walks::ask` ─────────────────────────────────────────────────────

#[derive(Default, Debug)]
struct Tally {
    asked: usize,
    yes: usize,
    no: usize,
    unknown: usize,
    not_linear: usize,
}

/// A `sum` beside a `max` or a `min`, or two of those: the one kind of question that is not linear,
/// so the check may be unable to say that it does not occur, though it finds the requests that give
/// it among the small collections.
fn not_linear(s: &S, which: &[usize]) -> bool {
    let extremes = which.iter().filter(|i| s.reads[**i].does == "extreme").count();
    extremes > 1 || (extremes == 1 && which.iter().any(|i| s.reads[*i].does == "sum"))
}

fn put(s: &S, walks: &Walks, which: &[usize], combo: &[f64]) -> Possible {
    let mut asks: Vec<(String, Ask)> = Vec::new();
    let mut ineqs: Vec<Ineq> = Vec::new();
    for (i, v) in which.iter().zip(combo) {
        let r = &s.reads[*i];
        match r.sort {
            Sort::Truth => asks.push((r.name.clone(), Ask::Truth(*v == 1.0))),
            Sort::Word => asks.push((r.name.clone(), Ask::Words(vec![r.values[*v as usize].to_string()]))),
            Sort::Number => {
                let d = Lin::var(&r.name).plus(&Lin::con(Rat::zero().sub(rat(*v))));
                ineqs.push(d.clone().le(false));
                ineqs.push(d.ge(false));
            }
        }
    }
    walks.ask(&asks, &ineqs)
}

/// What each read is asked to come to: everything seen, and past both ends of its range.
fn asked_values(r: &R, seen: &[Vec<f64>], i: usize) -> Vec<f64> {
    match r.sort {
        Sort::Truth => vec![0.0, 1.0],
        Sort::Word => (0..r.values.len()).map(|v| v as f64).collect(),
        Sort::Number => {
            let mut keys: BTreeSet<i64> = seen.iter().map(|v| key(v[i])).collect();
            if let Some(most) = keys.iter().last().copied() {
                keys.insert(most + 1000);
            }
            if let Some((lo, hi)) = r.range {
                for x in [lo - 1.0, lo, hi, hi + 1.0] {
                    keys.insert(key(x));
                }
            }
            keys.into_iter().map(|k| k as f64 / 1000.0).collect()
        }
    }
}

fn hold_asks(s: &S, tmp: &Path) -> Tally {
    let every: Vec<usize> = (0..s.reads.len()).collect();
    let src = s.rule(&every, None, &s.catch_all(0));
    let path = s.path(tmp);
    let (f, c) = rulec::prepare(&src, &path).unwrap_or_else(|d| panic!("{}: the rule does not read: {d:#?}\n{src}", s.tag));
    let walks = Walks::of(&f, &c, &path);
    let reqs = s.requests();
    assert!(!reqs.is_empty(), "{}: no request", s.tag);
    let seen = s.values(&reqs, &every);
    assert!(!seen.is_empty(), "{}: no request comes through the door", s.tag);
    let candidates: Vec<Vec<f64>> = s.reads.iter().enumerate().map(|(i, r)| asked_values(r, &seen, i)).collect();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in 0..s.reads.len() {
        groups.push(vec![i]);
        for j in i + 1..s.reads.len() {
            groups.push(vec![i, j]);
        }
    }
    let mut place = 0usize;
    for i in 0..s.reads.len() {
        for j in i + 1..s.reads.len() {
            for k in j + 1..s.reads.len() {
                place += 1;
                if s.triples != 0 && place.is_multiple_of(s.triples) {
                    groups.push(vec![i, j, k]);
                }
            }
        }
    }
    let mut t = Tally::default();
    for which in &groups {
        let mut combos: Vec<Vec<f64>> = vec![Vec::new()];
        for i in which {
            let mut next = Vec::with_capacity(combos.len() * candidates[*i].len());
            for c in &combos {
                for v in &candidates[*i] {
                    let mut n = c.clone();
                    n.push(*v);
                    next.push(n);
                }
            }
            combos = next;
        }
        for combo in combos {
            let names: Vec<String> = which.iter().zip(&combo).map(|(i, v)| format!("{} = {v}", s.reads[*i].name)).collect();
            let tell = format!("{}: {}", s.tag, names.join(", "));
            let exists = seen.iter().any(|v| which.iter().zip(&combo).all(|(i, c)| key(v[*i]) == key(*c)));
            t.asked += 1;
            match put(s, &walks, which, &combo) {
                Possible::No(_) => {
                    t.no += 1;
                    assert!(!exists, "{tell}: the check says it does not occur, and a request gives it");
                }
                Possible::Yes(ex) => {
                    t.yes += 1;
                    let q = s.request_of(&ex, &tell);
                    for (i, want) in which.iter().zip(&combo) {
                        let got = (s.reads[*i].eval)(&q);
                        assert_eq!(key(got), key(*want), "{tell}: the example {q:?} gives `{}` = {got}", s.reads[*i].name);
                    }
                    if let Some(v) = s.valid
                        && s.plain.iter().all(|f| q.top.contains_key(f.name))
                    {
                        assert!(v(&q), "{tell}: the contract refuses the example {q:?}");
                    }
                    assert!(!s.exact || exists, "{tell}: the check gives an example, and no request written out gives it");
                }
                Possible::Unknown => {
                    t.unknown += 1;
                    let relaxed = not_linear(s, which);
                    assert!(!s.exact || (relaxed && !exists), "{tell}: the check cannot say, on a scenario it reads all of (exists = {exists})");
                    if relaxed {
                        t.not_linear += 1;
                    }
                }
            }
        }
    }
    t
}

// ── The second: E123 ───────────────────────────────────────────────────────────────────────

fn diags(src: &str, path: &str) -> Vec<Diag> {
    i18n::with(Lang::En, || rulec::check_source(src, path))
}

/// Every `constraint` over two numbers: E123 comes exactly where a request breaks it.
fn hold_e123(s: &S, tmp: &Path) -> (usize, usize) {
    let (mut fired_n, mut quiet_n) = (0, 0);
    let reqs = s.requests();
    let path = s.path(tmp);
    for a in s.numbers() {
        for b in s.numbers() {
            if a == b || s.reads[a].unit != s.reads[b].unit {
                continue;
            }
            let (ra, rb) = (&s.reads[a], &s.reads[b]);
            let k = format!("constraint {} <= {}", ra.name, rb.name);
            let src = s.rule(&[a, b], Some(&k), &s.catch_all(a));
            let ds = diags(&src, &path);
            assert!(!ds.iter().any(|d| d.code == "E018"), "{}: {k} is not a comparison: {ds:#?}", s.tag);
            let fired = ds.iter().find(|d| d.code == "E123");
            let vals = s.values(&reqs, &[a, b]);
            let breaker = vals.iter().find(|v| key(v[a]) > key(v[b]));
            match (fired, breaker) {
                (Some(d), _) => {
                    fired_n += 1;
                    let said = d.notes.join("\n");
                    assert!(said.contains("passes the contract's validation"), "{}: {k}: {said}", s.tag);
                    if !s.exact && breaker.is_none() {
                        // past what is written out: the check has to show a request that gives it
                        let (f, c) = rulec::prepare(&src, &path).unwrap();
                        let walks = Walks::of(&f, &c, &path);
                        let d = Lin::var(&ra.name).plus(&Lin::var(&rb.name).scale(Rat::int(-1)));
                        let Possible::Yes(ex) = walks.ask(&[], &[d.ge(true)]) else {
                            panic!("{}: {k}: E123 and no example", s.tag);
                        };
                        let q = s.request_of(&ex, &k);
                        assert!(key((ra.eval)(&q)) > key((rb.eval)(&q)), "{}: {k}: the example {q:?} does not break it", s.tag);
                    }
                    assert!(!s.exact || breaker.is_some(), "{}: {k}: E123, and no request breaks it", s.tag);
                }
                (None, Some(v)) => {
                    quiet_n += 1;
                    assert!(!s.exact, "{}: {k}: a request breaks it ({} = {}, {} = {}), and there is no E123: {ds:#?}", s.tag, ra.name, v[a], rb.name, v[b]);
                }
                (None, None) => {}
            }
        }
    }
    (fired_n, quiet_n)
}

// ── The third and fourth: W133 and the note on E101 ────────────────────────────────────────

type Cell = (String, Box<dyn Fn(f64) -> bool>);

/// The cells a row can ask of a read. The last two parts of `parts` cut its values in two.
fn cells_of(r: &R) -> Vec<Cell> {
    match r.sort {
        Sort::Truth => vec![("true".to_string(), Box::new(|v| v == 1.0)), ("false".to_string(), Box::new(|v| v == 0.0))],
        Sort::Word => (0..r.values.len())
            .map(|i| {
                let c: Cell = (r.values[i].to_string(), Box::new(move |v| v == i as f64));
                c
            })
            .collect(),
        Sort::Number => {
            let (lo, hi) = r.range.unwrap_or((0.0, 9.0));
            let mid = lo.max(0.0) + 1.0;
            let mut out: Vec<Cell> = vec![
                (r.cell("", lo), Box::new(move |v| key(v) == key(lo))),
                (r.cell(">", lo), Box::new(move |v| key(v) > key(lo))),
                (r.cell(">=", mid), Box::new(move |v| key(v) >= key(mid))),
                (r.cell("<=", mid), Box::new(move |v| key(v) <= key(mid))),
            ];
            if hi > mid {
                out.push((r.cell("", hi), Box::new(move |v| key(v) == key(hi))));
            }
            out
        }
    }
}

/// Whether the contract's check stopped the rule, so the tables were not looked at. A `range`
/// narrower than what the contract lets through does that (E122).
fn stopped(ds: &[Diag]) -> bool {
    ds.iter().any(|d| matches!(d.code, "E120" | "E121" | "E122" | "E123"))
}

/// Every row over two reads, with a row under it that takes the rest: W133 comes exactly where no
/// request reaches the row.
fn hold_rows(s: &S, tmp: &Path) -> (usize, usize) {
    let reqs = s.requests();
    let path = s.path(tmp);
    let (mut warned, mut told) = (0, 0);
    for a in 0..s.reads.len() {
        for b in a + 1..s.reads.len() {
            let vals = s.values(&reqs, &[a, b]);
            for (ca, ha) in cells_of(&s.reads[a]) {
                for (cb, hb) in cells_of(&s.reads[b]) {
                    let table = format!(
                        "table k\npolicy first\n| {} | {} | -> x : number |\n| {ca} | {cb} | 1 |\n| - | - | 0 |\n",
                        s.reads[a].name, s.reads[b].name
                    );
                    let src = s.rule(&[a, b], None, &table);
                    let ds = diags(&src, &path);
                    if stopped(&ds) {
                        continue;
                    }
                    let w = ds.iter().any(|d| d.code == "W133" && d.row == Some(1));
                    let other = ds.iter().any(|d| matches!(d.code, "E102" | "W123" | "W124") && d.row == Some(1));
                    let reached = vals.iter().any(|v| ha(v[a]) && hb(v[b]));
                    let tell = format!("{}: `{}` {ca}, `{}` {cb}", s.tag, s.reads[a].name, s.reads[b].name);
                    assert!(!(w && reached), "{tell}: W133, and a request reaches the row");
                    assert!(!(w && other), "{tell}: W133 on a row something else already told");
                    if w {
                        warned += 1;
                    }
                    if other {
                        told += 1;
                    }
                    if s.exact && !reached && !other && !not_linear(s, &[a, b]) {
                        assert!(w, "{tell}: no request reaches the row, and there is no W133: {ds:#?}");
                    }
                }
            }
        }
    }
    (warned, told)
}

/// What E101's example says of a column, as this file's numbers.
fn witness_value(gap: &Diag, name: &str) -> f64 {
    let Some((_, v)) = gap.witness.inputs.iter().find(|(n, _)| n == name) else { panic!("E101's example has no `{name}`: {:?}", gap.witness.inputs) };
    let text = v.text();
    match text.as_str() {
        "true" => 1.0,
        "false" => 0.0,
        t => {
            let digits: String = t.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-')).collect();
            digits.parse().unwrap_or(f64::NAN)
        }
    }
}

/// Every table over two reads that leaves one combination out: the note on E101 says it never
/// occurs exactly where it does not.
fn hold_gaps(s: &S, tmp: &Path) -> (usize, usize) {
    let reqs = s.requests();
    let path = s.path(tmp);
    let (mut with_note, mut without) = (0, 0);
    let split = |r: &R| -> Vec<Cell> {
        match r.sort {
            Sort::Number => {
                let (lo, _) = r.range.unwrap_or((0.0, 9.0));
                vec![(r.cell("", lo), Box::new(move |v| key(v) == key(lo))), (r.cell(">", lo), Box::new(move |v| key(v) > key(lo)))]
            }
            _ => cells_of(r),
        }
    };
    for a in 0..s.reads.len() {
        for b in a + 1..s.reads.len() {
            let vals = s.values(&reqs, &[a, b]);
            let (cas, cbs) = (split(&s.reads[a]), split(&s.reads[b]));
            for left in 0..cas.len() * cbs.len() {
                let mut rows = String::new();
                for (i, (ca, _)) in cas.iter().enumerate() {
                    for (j, (cb, _)) in cbs.iter().enumerate() {
                        if i * cbs.len() + j != left {
                            rows.push_str(&format!("| {ca} | {cb} | 1 |\n"));
                        }
                    }
                }
                let table = format!("table k\npolicy unique\n| {} | {} | -> x : number |\n{rows}", s.reads[a].name, s.reads[b].name);
                let src = s.rule(&[a, b], None, &table);
                let ds = diags(&src, &path);
                let (ca, ha) = &cas[left / cbs.len()];
                let (cb, hb) = &cbs[left % cbs.len()];
                let tell = format!("{}: `{}` {ca}, `{}` {cb}", s.tag, s.reads[a].name, s.reads[b].name);
                let Some(gap) = ds.iter().find(|d| d.code == "E101") else {
                    continue; // the contract's check stopped first, or a range already took it
                };
                let note = gap.notes.iter().any(|n| n.contains("never comes out of the definitions"));
                let reached = vals.iter().any(|v| ha(v[a]) && hb(v[b]));
                // A gap that occurs is shown ahead of one that does not, so the note says it of all
                // the table left out; and with no note the example is what occurs.
                let shown: Vec<f64> = [a, b].iter().map(|i| witness_value(gap, &s.reads[*i].name)).collect();
                let at_example = vals.iter().any(|v| key(v[a]) == key(shown[0]) && key(v[b]) == key(shown[1]));
                let say = format!("{tell}: {:#?}", gap.notes);
                assert!(!(note && reached), "the note says it never occurs, and a request reaches the part of the table left out: {say}");
                if s.exact && !not_linear(s, &[a, b]) {
                    assert!(note || reached, "nothing gives it, and E101 has no note: {say}");
                    assert!(note || at_example, "E101 has no note, and nothing gives its example: {say}");
                }
                if note {
                    with_note += 1;
                } else {
                    without += 1;
                }
            }
        }
    }
    (with_note, without)
}

// ── The sixth: `never` lines, held to the same requests ────────────────────────────────────

/// A cell of [`cells_of`] as a condition of a `never` line.
fn never_cond(r: &R, cell: &str) -> String {
    if cell.starts_with(['<', '>']) { format!("{} {cell}", r.name) } else { format!("{} = {cell}", r.name) }
}

#[derive(Default)]
struct Lines {
    taken: usize,
    occurs: usize,
    unshown: usize,
    mutated: usize,
    rechecked: usize,
}

/// How many taken lines of a scenario go through the re-checker, which is a process each.
const RECHECKED: usize = 10;

/// The certificate of a rule with a line taken, through `tools/recheck.py` against the rule's text.
fn recheck_line(s: &S, tmp: &Path, src: &str, path: &str) {
    let (f, c) = i18n::with(Lang::En, || rulec::prepare(src, path)).unwrap_or_else(|d| panic!("{}: {d:#?}", s.tag));
    let cert = i18n::with(Lang::En, || rulec::cert::certificate(&f, &c, src, path));
    let at = tmp.join("cert.json");
    std::fs::write(path, src).unwrap();
    std::fs::write(&at, cert).unwrap();
    let tool = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/recheck.py");
    let o = Command::new("python3").arg("-B").arg(&tool).arg("--rule").arg(path).arg(&at).output().expect("cannot start python3");
    assert!(o.status.success(), "{}: the re-checker refuses a line the check took:\n{src}\n{}{}", s.tag, String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
}

/// Every line over one read and over two: a line the check takes does not occur in any request
/// written out, a line it stops with E142 does occur and the request it shows gives it, and where the
/// requests are all there are, the check is exact.
fn hold_nevers(s: &S, tmp: &Path) -> Lines {
    let reqs = s.requests();
    let path = s.path(tmp);
    let mut t = Lines::default();
    let python = have_python();
    let all_reads: Vec<usize> = (0..s.reads.len()).collect();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for a in 0..s.reads.len() {
        groups.push(vec![a]);
        for b in a + 1..s.reads.len() {
            // A line over an input and a summary is a mistake of its own (E141).
            if s.reads[a].summary.is_some() == s.reads[b].summary.is_some() {
                groups.push(vec![a, b]);
            }
        }
    }
    for which in &groups {
        let vals = s.values(&reqs, which);
        let heads: Vec<&str> = which.iter().map(|i| s.reads[*i].name.as_str()).collect();
        let table = format!("table k\npolicy first\n| {} | -> x : number |\n| {} | 1 |\n", heads.join(" | "), vec!["-"; which.len()].join(" | "));
        let mut combos: Vec<Vec<usize>> = vec![Vec::new()];
        let cells: Vec<Vec<Cell>> = which.iter().map(|i| cells_of(&s.reads[*i])).collect();
        for cs in &cells {
            combos = combos.iter().flat_map(|c| (0..cs.len()).map(move |k| c.iter().copied().chain([k]).collect::<Vec<usize>>())).collect();
        }
        for combo in combos {
            let conds: Vec<String> = which.iter().zip(&cells).zip(&combo).map(|((i, cs), k)| never_cond(&s.reads[*i], &cs[*k].0)).collect();
            let line = format!("never {}", conds.join(", "));
            // With every read in the rule, as the questions are put: a count's range can be what
            // holds a collection to the size written out. Where one of them stops the rule, with
            // only the line's own, and the check need not be exact.
            let every = s.rule(&all_reads, Some(&line), &table);
            let ds = diags(&every, &path);
            let whole = !ds.iter().any(|d| matches!(d.code, "E120" | "E121" | "E122" | "E123" | "E020"));
            let (src, ds) = if whole {
                (every, ds)
            } else {
                let src = s.rule(which, Some(&line), &table);
                let ds = diags(&src, &path);
                (src, ds)
            };
            if stopped(&ds) || ds.iter().any(|d| d.code == "E020") {
                continue;
            }
            let tell = format!("{}: `{line}`", s.tag);
            let hits = |v: &[f64]| which.iter().zip(&cells).zip(&combo).all(|((i, cs), k)| (cs[*k].1)(v[*i]));
            let reached = vals.iter().any(|v| hits(v));
            assert!(!ds.iter().any(|d| d.code == "E141"), "{tell}: {ds:#?}");
            if ds.iter().any(|d| d.code == "E142") {
                t.occurs += 1;
                assert!(!(s.exact && whole) || reached, "{tell}: E142, and no request gives it");
                // The request it shows has to give the combination, and pass the contract.
                let ex = i18n::with(Lang::En, || {
                    let (f, c) = rulec::prepare(&src, &path).unwrap();
                    let walks = Walks::of(&f, &c, &path);
                    match walks.never_answer(&f.nevers[0]) {
                        Possible::Yes(ex) => ex,
                        _ => panic!("{tell}: E142, and no example"),
                    }
                });
                let q = s.request_of(&ex, &tell);
                let got: Vec<f64> = s.reads.iter().enumerate().map(|(i, r)| if which.contains(&i) { (r.eval)(&q) } else { f64::NAN }).collect();
                assert!(hits(&got), "{tell}: the request {q:?} does not give it: {got:?}");
                if let Some(v) = s.valid
                    && s.plain.iter().all(|f| q.top.contains_key(f.name))
                {
                    assert!(v(&q), "{tell}: the contract refuses the request {q:?}");
                }
            } else if ds.iter().any(|d| d.code == "E143") {
                t.unshown += 1;
                assert!(!s.exact || !whole || reached || not_linear(s, which), "{tell}: nothing gives it, and the check cannot show that: {ds:#?}");
            } else {
                t.taken += 1;
                assert!(!reached, "{tell}: the check takes the line, and a request gives it");
                if python && t.rechecked < RECHECKED && !ds.iter().any(|d| d.severity == rulec::diag::Severity::Error) {
                    t.rechecked += 1;
                    recheck_line(s, tmp, &src, &path);
                }
            }
        }
    }
    t
}

/// A table that leaves one combination out passes with the line E101's note shows, and with a line
/// one condition off it does not: the seeded mistake is stopped by E142, by E143, or by the gap it
/// left where it was.
fn hold_note_lines(s: &S, tmp: &Path, t: &mut Lines) {
    let path = s.path(tmp);
    let split = |r: &R| -> Vec<Cell> {
        match r.sort {
            Sort::Number => {
                let (lo, _) = r.range.unwrap_or((0.0, 9.0));
                vec![(r.cell("", lo), Box::new(move |v| key(v) == key(lo))), (r.cell(">", lo), Box::new(move |v| key(v) > key(lo)))]
            }
            _ => cells_of(r),
        }
    };
    let errors = |ds: &[Diag]| ds.iter().any(|d| d.severity == rulec::diag::Severity::Error);
    for a in 0..s.reads.len() {
        for b in a + 1..s.reads.len() {
            if s.reads[a].summary.is_some() != s.reads[b].summary.is_some() {
                continue;
            }
            let (cas, cbs) = (split(&s.reads[a]), split(&s.reads[b]));
            for left in 0..cas.len() * cbs.len() {
                let mut rows = String::new();
                for (i, (ca, _)) in cas.iter().enumerate() {
                    for (j, (cb, _)) in cbs.iter().enumerate() {
                        if i * cbs.len() + j != left {
                            rows.push_str(&format!("| {ca} | {cb} | 1 |\n"));
                        }
                    }
                }
                let table = format!("table k\npolicy unique\n| {} | {} | -> x : number |\n{rows}", s.reads[a].name, s.reads[b].name);
                let ds = diags(&s.rule(&[a, b], None, &table), &path);
                let Some(gap) = ds.iter().find(|d| d.code == "E101") else { continue };
                let said = gap.notes.join("\n");
                let Some(line) = said.split("with this line, and no row: `").nth(1).and_then(|x| x.split('`').next()) else { continue };
                let with_line = diags(&s.rule(&[a, b], Some(line), &table), &path);
                assert!(!errors(&with_line), "{}: the line E101 shows does not pass: `{line}`: {with_line:#?}", s.tag);
                t.mutated += 1;
                // The same gap with the line naming another cell of the first read.
                let (i, j) = (left / cbs.len(), left % cbs.len());
                let off = format!("never {}, {}", never_cond(&s.reads[a], &cas[(i + 1) % cas.len()].0), never_cond(&s.reads[b], &cbs[j].0));
                let with_off = diags(&s.rule(&[a, b], Some(&off), &table), &path);
                assert!(errors(&with_off), "{}: `{off}` over a gap at `{}`, `{}` passes: {with_off:#?}", s.tag, cas[i].0, cbs[j].0);
            }
        }
    }
}

// ── The fifth: the evaluator, held to the generated Python ─────────────────────────────────

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success())
}

const READ_BACK: &str = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("t", sys.argv[1])
m = importlib.util.module_from_spec(spec)
sys.modules["t"] = m
spec.loader.exec_module(m)
def shown(x):
    try:
        return str(float(x))
    except (TypeError, ValueError):
        return str(getattr(x, "value", x))
m.t = lambda *a: a
for order in json.load(open(sys.argv[2], encoding="utf-8")):
    try:
        print(",".join(shown(x) for x in m.t_from(order)))
    except Exception as e:
        print("!", type(e).__name__, e)
"#;

const READ_SUMMARY: &str = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("t", sys.argv[1])
m = importlib.util.module_from_spec(spec)
sys.modules["t"] = m
spec.loader.exec_module(m)
for lines in json.load(open(sys.argv[2], encoding="utf-8")):
    try:
        print(float(m.t([m.Element(**e) for e in lines])))
    except Exception as e:
        print("!", type(e).__name__, e)
"#;

fn value_json(v: &V) -> String {
    match v {
        V::W(w) => format!("\"{w}\""),
        V::N(n) => lit(*n),
        V::T(b) => b.to_string(),
    }
}

#[derive(Default)]
struct Node {
    leaf: Option<String>,
    kids: BTreeMap<String, Node>,
}

impl Node {
    fn set(&mut self, dotted: &str, text: String) {
        match dotted.split_once('.') {
            Some((a, b)) => self.kids.entry(a.to_string()).or_default().set(b, text),
            None => self.kids.entry(dotted.to_string()).or_default().leaf = Some(text),
        }
    }

    fn json(&self) -> String {
        match &self.leaf {
            Some(t) => t.clone(),
            None => format!("{{{}}}", self.kids.iter().map(|(k, n)| format!("\"{k}\":{}", n.json())).collect::<Vec<_>>().join(",")),
        }
    }
}

fn element_json(s: &S, e: &El) -> String {
    let mut n = Node::default();
    for (k, v) in e {
        if !s.left_out.contains(v) {
            n.set(k, value_json(v));
        }
    }
    n.json()
}

fn request_json(s: &S, q: &Req) -> String {
    let mut n = Node::default();
    for (k, v) in &q.top {
        if !s.left_out.contains(v) {
            n.set(k, value_json(v));
        }
    }
    for (k, es) in &q.walks {
        n.set(k, format!("[{}]", es.iter().map(|e| element_json(s, e)).collect::<Vec<_>>().join(",")));
    }
    n.json()
}

fn python(tmp: &Path, script: &str, code: &str, data: &str) -> Vec<String> {
    std::fs::write(tmp.join("read_back.py"), script).unwrap();
    std::fs::write(tmp.join("generated_t.py"), code).unwrap();
    std::fs::write(tmp.join("requests.json"), data).unwrap();
    // `-B`: two generated files of one size written in one second would share a stale `.pyc`.
    let o = Command::new("python3").arg("-B").arg(tmp.join("read_back.py")).arg(tmp.join("generated_t.py")).arg(tmp.join("requests.json")).output().expect("cannot start python3");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).lines().map(str::to_string).collect()
}

fn hold_python(s: &S, tmp: &Path) -> usize {
    if s.no_read_back || !ready(Need::Python, have_python, "python3 is not there") {
        return 0;
    }
    let reqs = s.requests();
    let path = s.path(tmp);
    if s.root.is_empty() {
        // A summary comes out only through the rule, so one rule for each.
        let mut told = 0;
        for (i, r) in s.reads.iter().enumerate() {
            let src = s.rule(&[i], None, &format!("result x = {}\n", r.name));
            let (f, c) = rulec::prepare(&src, &path).unwrap_or_else(|d| panic!("{}: {d:#?}\n{src}", s.tag));
            let code = rulec::codegen::Gen::new(&f, &c, &src, &path).python();
            let key_name = s.colls[0].key;
            let data = format!("[{}]", reqs.iter().map(|q| format!("[{}]", walk(q, key_name).iter().map(|e| element_json(s, e)).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(","));
            let got = python(tmp, READ_SUMMARY, &code, &data);
            assert_eq!(got.len(), reqs.len(), "{}", s.tag);
            for (q, line) in reqs.iter().zip(&got) {
                let want = (r.eval)(q);
                assert_eq!(line, &format!("{:?}", want.floor()), "{}: `{}` on {q:?}", s.tag, r.name);
                told += 1;
            }
        }
        return told;
    }
    let every: Vec<usize> = (0..s.reads.len()).collect();
    let src = s.rule(&every, None, &s.catch_all(0));
    let (f, c) = rulec::prepare(&src, &path).unwrap_or_else(|d| panic!("{}: {d:#?}\n{src}", s.tag));
    let code = rulec::codegen::Gen::new(&f, &c, &src, &path).python();
    let data = format!("[{}]", reqs.iter().map(|q| request_json(s, q)).collect::<Vec<_>>().join(","));
    let got = python(tmp, READ_BACK, &code, &data);
    assert_eq!(got.len(), reqs.len(), "{}", s.tag);
    for (q, line) in reqs.iter().zip(&got) {
        let want: Vec<String> = s
            .reads
            .iter()
            .map(|r| {
                let v = (r.eval)(q);
                if r.sort == Sort::Word { r.values[v as usize].to_string() } else { format!("{v:?}") }
            })
            .collect();
        assert_eq!(line, &want.join(","), "{}: the generated code and this file differ on {}", s.tag, request_json(s, q));
    }
    reqs.len()
}

fn hold(s: S) {
    let tmp = TempDir::new(&format!("walks-{}", s.tag));
    if let Some((name, text)) = &s.file {
        std::fs::write(tmp.path().join(name), text).unwrap();
    }
    let t = hold_asks(&s, tmp.path());
    let (fired, quiet) = hold_e123(&s, tmp.path());
    let (warned, told) = hold_rows(&s, tmp.path());
    let (noted, plain) = hold_gaps(&s, tmp.path());
    let read_back = hold_python(&s, tmp.path());
    let mut lines = hold_nevers(&s, tmp.path());
    hold_note_lines(&s, tmp.path(), &mut lines);
    println!(
        "compared {} never: taken {}, occurs {}, cannot be shown {}, mutated {}, re-checked {}",
        s.tag, lines.taken, lines.occurs, lines.unshown, lines.mutated, lines.rechecked
    );
    println!(
        "compared {}: {} requests; asked {} (yes {}, no {}, unknown {}, of them a sum beside an extreme {}); E123 fired {fired}, quiet {quiet}; W133 {warned}, told otherwise {told}; E101 with the note {noted}, without {plain}; Python {read_back}",
        s.tag,
        s.requests().len(),
        t.asked,
        t.yes,
        t.no,
        t.unknown,
        t.not_linear
    );
}

// ── The scenarios ──────────────────────────────────────────────────────────────────────────

const JSON_SHAPE: &str = r##"shape order = jsonschema "order.json" "#/$defs/Order""##;
const PROTO_SHAPE: &str = r##"shape order = proto "order.proto" shop.v1.Order"##;

fn orders_json(lines_rule: &str, line_props: &str, required: &str) -> String {
    format!(
        r##"{{"$defs":{{"Order":{{"type":"object","properties":{{"lines":{{"type":"array","items":{{"$ref":"#/$defs/Line"}}{lines_rule}}}}},"required":["lines"]}},"Line":{{"type":"object","properties":{{{line_props}}},"required":[{required}]}}}}}}"##
    )
}

const CATEGORY: &str = r#""category":{"type":"string","enum":["tool","part","service"]}"#;

fn word_reads() -> Vec<R> {
    let wide = (0.0, 9.0);
    vec![
        any("has_tool", "order.lines", "category = tool", |e| word(e, "category") == "tool"),
        any("has_part", "order.lines", "category = part", |e| word(e, "category") == "part"),
        all("all_parts", "order.lines", "category = part", |e| word(e, "category") == "part"),
        all("no_tools", "order.lines", "category = not: tool", |e| word(e, "category") != "tool"),
        count("tools", "order.lines", "category = tool", |e| word(e, "category") == "tool", wide),
        count("non_parts", "order.lines", "category = not: part", |e| word(e, "category") != "part", wide),
        count("tools_or_parts", "order.lines", "category = tool, part", |e| matches!(word(e, "category"), "tool" | "part"), wide),
        count("size", "order.lines", "", |_| true, wide),
    ]
}

/// The materials' form: an enum in the contract, and a limit on the collection.
#[test]
fn an_enum_and_a_limit() {
    hold(S {
        tag: "enum-limit",
        file: Some(("order.json", orders_json(r#","maxItems":3"#, CATEGORY, r#""category""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("category", words(&["tool", "part", "service"]))], least: 0, most: 3, capped: true }],
        reads: word_reads(),
        exact: true,
        triples: 5,
        ..S::default()
    });
}

/// No enum, so a string is any string; and with no limit, the check's examples can go past what is
/// written out, and are worked out on their own.
#[test]
fn any_string_and_no_limit() {
    hold(S {
        tag: "any-string",
        file: Some(("order.json", orders_json("", r#""category":{"type":"string"}"#, r#""category""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![sample("category", words(&["tool", "part", "other"]))], least: 0, most: 3, capped: false }],
        reads: word_reads(),
        exact: false,
        triples: 11,
        ..S::default()
    });
}

/// An element that has to be there, and a limit: the empty collection goes away, and with it what
/// only the empty collection gives.
#[test]
fn at_least_one_element() {
    hold(S {
        tag: "at-least-one",
        file: Some(("order.json", orders_json(r#","minItems":1,"maxItems":2"#, CATEGORY, r#""category""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("category", words(&["tool", "part", "service"]))], least: 1, most: 2, capped: true }],
        reads: word_reads(),
        exact: true,
        triples: 5,
        ..S::default()
    });
}

/// A value in a `where` the contract never lets through makes that class empty.
#[test]
fn a_where_value_outside_the_enum() {
    hold(S {
        tag: "where-outside",
        file: Some(("order.json", orders_json(r#","maxItems":2"#, CATEGORY, r#""category""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("category", words(&["tool", "part", "service"]))], least: 0, most: 2, capped: true }],
        reads: vec![
            any("has_ghost", "order.lines", "category = ghost", |e| word(e, "category") == "ghost"),
            all("all_ghosts", "order.lines", "category = ghost", |e| word(e, "category") == "ghost"),
            count("ghosts", "order.lines", "category = ghost", |e| word(e, "category") == "ghost", (0.0, 9.0)),
            count("size", "order.lines", "", |_| true, (0.0, 9.0)),
        ],
        exact: true,
        triples: 1,
        ..S::default()
    });
}

/// `range` on the reads: the door refuses a request outside it, and only the requests inside count.
#[test]
fn a_range_that_cuts() {
    hold(S {
        tag: "range-cuts",
        file: Some(("order.json", orders_json(r#","maxItems":3"#, CATEGORY, r#""category""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("category", words(&["tool", "part", "service"]))], least: 0, most: 3, capped: true }],
        reads: vec![
            any("has_tool", "order.lines", "category = tool", |e| word(e, "category") == "tool"),
            all("all_parts", "order.lines", "category = part", |e| word(e, "category") == "part"),
            count("tools", "order.lines", "category = tool", |e| word(e, "category") == "tool", (0.0, 1.0)),
            count("size", "order.lines", "", |_| true, (1.0, 2.0)),
            count("parts", "order.lines", "category = part", |e| word(e, "category") == "part", (1.0, 3.0)),
        ],
        exact: true,
        triples: 1,
        ..S::default()
    });
}

fn number_reads() -> Vec<R> {
    let wide = (0.0, 20.0);
    vec![
        any("has_big", "order.lines", "quantity >= 3", |e| num(e, "quantity") >= 3.0),
        all("all_small", "order.lines", "quantity < 3", |e| num(e, "quantity") < 3.0),
        count("twos", "order.lines", "quantity = 2", |e| num(e, "quantity") == 2.0, wide),
        count("not_twos", "order.lines", "quantity = not: 2", |e| num(e, "quantity") != 2.0, wide),
        count("big", "order.lines", "quantity >= 2", |e| num(e, "quantity") >= 2.0, wide),
        sum("total", "order.lines", "quantity", "", |_| true, wide),
        sum("big_total", "order.lines", "quantity", "quantity > 1", |e| num(e, "quantity") > 1.0, wide),
        max("largest", "order.lines", "quantity", "", |_| true, 0.0, wide),
        min("smallest", "order.lines", "quantity", "", |_| true, 9.0, wide),
        max("small_largest", "order.lines", "quantity", "quantity <= 2", |e| num(e, "quantity") <= 2.0, 2.0, wide),
        min("big_smallest", "order.lines", "quantity", "quantity >= 2", |e| num(e, "quantity") >= 2.0, 1.0, wide),
    ]
}

/// Numbers in `where`, `sum`, and `max` and `min` with an `empty` outside and inside the values.
#[test]
fn numbers_in_where_sum_and_extremes() {
    hold(S {
        tag: "numbers",
        file: Some(("order.json", orders_json(r#","maxItems":3"#, r#""quantity":{"type":"integer","minimum":1,"maximum":3}"#, r#""quantity""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("quantity", nums(&[1.0, 2.0, 3.0]))], least: 0, most: 3, capped: true }],
        reads: number_reads(),
        exact: true,
        triples: 17,
        ..S::default()
    });
}

type ElTest = fn(&El) -> bool;

fn mixed_reads(tool: &'static str, part: &'static str) -> Vec<R> {
    // `fn` pointers cannot capture, so the two names travel through the rule's text alone, and the
    // scenarios that use this share the two tests below.
    let (is_tool, is_part): (ElTest, ElTest) = if tool.starts_with("CATEGORY") {
        (|e| word(e, "category") == "CATEGORY_TOOL", |e| word(e, "category") == "CATEGORY_PART")
    } else {
        (|e| word(e, "category") == "tool", |e| word(e, "category") == "part")
    };
    let wide = (0.0, 20.0);
    vec![
        any("has_fragile", "order.lines", "fragile = true", |e| yes(e, "fragile")),
        all("none_fragile", "order.lines", "fragile = false", |e| !yes(e, "fragile")),
        count("fragile_lines", "order.lines", "fragile = true", |e| yes(e, "fragile"), wide),
        count("tools", "order.lines", &format!("category = {tool}"), is_tool, wide),
        sum("tool_total", "order.lines", "quantity", &format!("category = {tool}"), is_tool, wide),
        max("fragile_top", "order.lines", "quantity", "fragile = true", |e| yes(e, "fragile"), 0.0, wide),
        min("part_low", "order.lines", "quantity", &format!("category = {part}"), is_part, 5.0, wide),
    ]
}

/// Several fields at once, so the classes are a product.
#[test]
fn several_fields_in_a_json_schema() {
    hold(S {
        tag: "several-json",
        file: Some((
            "order.json",
            orders_json(
                r#","minItems":1,"maxItems":2"#,
                &format!(r#"{CATEGORY},"quantity":{{"type":"integer","minimum":1,"maximum":2}},"fragile":{{"type":"boolean"}}"#),
                r#""category","quantity","fragile""#,
            ),
        )),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll {
            key: "lines",
            fields: vec![closed("category", words(&["tool", "part", "service"])), closed("quantity", nums(&[1.0, 2.0])), closed("fragile", truths())],
            least: 1,
            most: 2,
            capped: true,
        }],
        reads: mixed_reads("tool", "part"),
        exact: true,
        triples: 7,
        ..S::default()
    });
}

const CATEGORY_PROTO: &str = "enum Category {\n  CATEGORY_UNSPECIFIED = 0;\n  CATEGORY_TOOL = 1;\n  CATEGORY_PART = 2;\n  CATEGORY_SERVICE = 3;\n}\n";

/// The same through a `.proto`: `repeated`, the enum's `in`, and the limits written as buf.validate.
#[test]
fn several_fields_in_a_proto() {
    let body = format!(
        "syntax = \"proto3\";\npackage shop.v1;\nimport \"buf/validate/validate.proto\";\n\n{CATEGORY_PROTO}\nmessage Line {{\n  Category category = 1 [(buf.validate.field).enum = {{in: [1, 2, 3]}}];\n  int32 quantity = 2 [(buf.validate.field).int32 = {{gte: 1, lte: 2}}];\n  bool fragile = 3;\n}}\n\nmessage Order {{\n  repeated Line lines = 1 [(buf.validate.field).repeated = {{min_items: 1, max_items: 2}}];\n}}\n"
    );
    hold(S {
        tag: "several-proto",
        file: Some(("order.proto", body)),
        head: PROTO_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll {
            key: "lines",
            fields: vec![closed("category", words(&["CATEGORY_TOOL", "CATEGORY_PART", "CATEGORY_SERVICE"])), closed("quantity", nums(&[1.0, 2.0])), closed("fragile", truths())],
            least: 1,
            most: 2,
            capped: true,
        }],
        reads: mixed_reads("CATEGORY_TOOL", "CATEGORY_PART"),
        exact: true,
        left_out: vec![V::T(false)],
        triples: 7,
        ..S::default()
    });
}

/// A `.proto` with no rule on its fields: an unset enum reads as its zero value and an unset
/// number as 0, and both count as values of the field.
#[test]
fn a_proto_with_no_rules_on_its_fields() {
    let body = format!(
        "syntax = \"proto3\";\npackage shop.v1;\nimport \"buf/validate/validate.proto\";\n\n{CATEGORY_PROTO}\nmessage Line {{\n  Category category = 1;\n  int32 quantity = 2;\n}}\n\nmessage Order {{\n  repeated Line lines = 1 [(buf.validate.field).repeated.max_items = 2];\n}}\n"
    );
    let wide = (-9.0, 20.0);
    hold(S {
        tag: "proto-open",
        file: Some(("order.proto", body)),
        head: PROTO_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll {
            key: "lines",
            fields: vec![sample("category", words(&["CATEGORY_UNSPECIFIED", "CATEGORY_TOOL", "CATEGORY_PART"])), sample("quantity", nums(&[0.0, 1.0, 2.0]))],
            least: 0,
            most: 2,
            capped: true,
        }],
        reads: vec![
            any("unset", "order.lines", "category = CATEGORY_UNSPECIFIED", |e| word(e, "category") == "CATEGORY_UNSPECIFIED"),
            all("all_tools", "order.lines", "category = CATEGORY_TOOL", |e| word(e, "category") == "CATEGORY_TOOL"),
            count("tools", "order.lines", "category = CATEGORY_TOOL", |e| word(e, "category") == "CATEGORY_TOOL", wide),
            count("positive", "order.lines", "quantity > 0", |e| num(e, "quantity") > 0.0, wide),
            sum("total", "order.lines", "quantity", "", |_| true, wide),
            max("largest", "order.lines", "quantity", "", |_| true, 0.0, wide),
        ],
        exact: false,
        left_out: vec![V::N(0.0), V::W("CATEGORY_UNSPECIFIED".to_string())],
        triples: 1,
        ..S::default()
    });
}

/// English first; here are the same reads with Japanese names, whose words are their own.
#[test]
fn japanese_names() {
    let contract = r##"{"$defs":{"注文":{"type":"object","properties":{"明細":{"type":"array","items":{"$ref":"#/$defs/明細"},"maxItems":3}},"required":["明細"]},"明細":{"type":"object","properties":{"区分":{"type":"string","enum":["工具","部品","作業"]},"数量":{"type":"integer","minimum":1,"maximum":2}},"required":["区分","数量"]}}}"##;
    let wide = (0.0, 20.0);
    hold(S {
        tag: "japanese",
        file: Some(("order.json", contract.to_string())),
        head: r##"shape 注文(order) = jsonschema "order.json" "#/$defs/注文""##.to_string(),
        root: "注文",
        colls: vec![Coll { key: "明細", fields: vec![closed("区分", words(&["工具", "部品", "作業"])), closed("数量", nums(&[1.0, 2.0]))], least: 0, most: 3, capped: true }],
        reads: vec![
            any("工具あり", "注文.明細", "区分 = 工具", |e| word(e, "区分") == "工具").alias("has_tool"),
            all("全部部品", "注文.明細", "区分 = 部品", |e| word(e, "区分") == "部品").alias("all_parts"),
            count("工具の数", "注文.明細", "区分 = 工具", |e| word(e, "区分") == "工具", wide).alias("tools"),
            count("部品以外", "注文.明細", "区分 = not: 部品", |e| word(e, "区分") != "部品", wide).alias("non_parts"),
            sum("数量の計", "注文.明細", "数量", "", |_| true, wide).alias("total"),
            max("最大数量", "注文.明細", "数量", "", |_| true, 0.0, wide).alias("largest"),
        ],
        exact: true,
        triples: 3,
        ..S::default()
    });
}

/// Summaries over a sequence the rule declares. The door stops the sequence at the top of a
/// `count`'s range, so it has a limit too, with no contract at all.
#[test]
fn summaries_over_a_sequence() {
    let wide = (0.0, 3.0);
    hold(S {
        tag: "elements",
        file: None,
        head: "elements lines\n  is_tool : bool\n  weight  : number  range >=1 <=2\n".to_string(),
        root: "",
        colls: vec![Coll { key: "lines", fields: vec![closed("is_tool", truths()), closed("weight", nums(&[1.0, 2.0]))], least: 0, most: 3, capped: true }],
        reads: vec![
            over_count("tools", "lines", "is_tool", |e| yes(e, "is_tool"), wide),
            over_count("others", "lines", "is_tool = false", |e| !yes(e, "is_tool"), wide),
            over_sum("total", "lines", "weight", (0.0, 99.0)),
            over_max("heaviest", "lines", "weight", "", |_| true, 0.0),
            over_min("lightest", "lines", "weight", "", |_| true, 9.0),
            over_max("heaviest_tool", "lines", "weight", "is_tool", |e| yes(e, "is_tool"), 0.0),
        ],
        exact: true,
        triples: 3,
        ..S::default()
    });
}

/// A value a rule reads beside the collection, and a condition across the two in the contract.
#[test]
fn a_value_beside_the_collection_and_a_condition_across_them() {
    let body = format!(
        "syntax = \"proto3\";\npackage shop.v1;\nimport \"buf/validate/validate.proto\";\n\n{CATEGORY_PROTO}\nmessage Line {{\n  Category category = 1 [(buf.validate.field).enum = {{in: [1, 2]}}];\n}}\n\nmessage Order {{\n  option (buf.validate.message).cel = {{id: \"size\", expression: \"this.lines.size() <= this.limit\"}};\n  int32 limit = 1 [(buf.validate.field).int32 = {{gte: 0, lte: 2}}];\n  bool express = 2;\n  repeated Line lines = 3 [(buf.validate.field).repeated.max_items = 3];\n}}\n"
    );
    let wide = (0.0, 9.0);
    hold(S {
        tag: "beside",
        file: Some(("order.proto", body)),
        head: PROTO_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("category", words(&["CATEGORY_TOOL", "CATEGORY_PART"]))], least: 0, most: 3, capped: true }],
        plain: vec![closed("limit", nums(&[0.0, 1.0, 2.0])), closed("express", truths())],
        valid: Some(|q| walk(q, "lines").len() as f64 <= num(&q.top, "limit")),
        reads: vec![
            plain_number("limit", "order.limit", wide),
            plain_truth("express", "order.express"),
            count("tools", "order.lines", "category = CATEGORY_TOOL", |e| word(e, "category") == "CATEGORY_TOOL", wide),
            count("size", "order.lines", "", |_| true, wide),
            any("has_tool", "order.lines", "category = CATEGORY_TOOL", |e| word(e, "category") == "CATEGORY_TOOL"),
        ],
        exact: true,
        left_out: vec![V::T(false), V::N(0.0)],
        triples: 1,
        ..S::default()
    });
}

/// Two collections, and an enum read beside them: reads of one collection say nothing of the
/// other.
#[test]
fn two_collections_and_an_enum_beside_them() {
    let contract = r##"{"$defs":{"Order":{"type":"object","properties":{"zone":{"type":"string","enum":["near","far"]},"lines":{"type":"array","maxItems":2,"items":{"$ref":"#/$defs/Line"}},"fees":{"type":"array","maxItems":2,"items":{"$ref":"#/$defs/Fee"}}},"required":["zone","lines","fees"]},"Line":{"type":"object","properties":{"category":{"type":"string","enum":["tool","part"]}},"required":["category"]},"Fee":{"type":"object","properties":{"amount":{"type":"integer","minimum":1,"maximum":2}},"required":["amount"]}}}"##;
    let wide = (0.0, 9.0);
    hold(S {
        tag: "two-collections",
        file: Some(("order.json", contract.to_string())),
        head: format!("{JSON_SHAPE}\n\nenum zone_kind = near | far"),
        root: "order",
        colls: vec![
            Coll { key: "lines", fields: vec![closed("category", words(&["tool", "part"]))], least: 0, most: 2, capped: true },
            Coll { key: "fees", fields: vec![closed("amount", nums(&[1.0, 2.0]))], least: 0, most: 2, capped: true },
        ],
        plain: vec![closed("zone", words(&["near", "far"]))],
        reads: vec![
            plain_word("zone", "order.zone", "zone_kind", &["near", "far"]),
            any("has_tool", "order.lines", "category = tool", |e| word(e, "category") == "tool"),
            all("all_parts", "order.lines", "category = part", |e| word(e, "category") == "part"),
            count("fee_count", "order.fees", "", |_| true, wide),
            sum("fee_total", "order.fees", "amount", "", |_| true, wide),
            count("tools", "order.lines", "category = tool", |e| word(e, "category") == "tool", wide),
        ],
        exact: true,
        triples: 3,
        ..S::default()
    });
}

/// A collection further down the object.
#[test]
fn paths_further_down() {
    let contract = r##"{"$defs":{"Order":{"type":"object","properties":{"box":{"$ref":"#/$defs/Box"}},"required":["box"]},"Box":{"type":"object","properties":{"lines":{"type":"array","maxItems":2,"items":{"$ref":"#/$defs/Line"}}},"required":["lines"]},"Line":{"type":"object","properties":{"quantity":{"type":"integer","minimum":1,"maximum":2}},"required":["quantity"]}}}"##;
    let wide = (0.0, 9.0);
    hold(S {
        tag: "further-down",
        file: Some(("order.json", contract.to_string())),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll {
            key: "box.lines",
            fields: vec![closed("quantity", nums(&[1.0, 2.0]))],
            least: 0,
            most: 2,
            capped: true,
        }],
        reads: vec![
            count("size", "order.box.lines", "", |_| true, wide),
            sum("total", "order.box.lines", "quantity", "", |_| true, wide),
            max("largest", "order.box.lines", "quantity", "", |_| true, 0.0, wide),
            count("twos", "order.box.lines", "quantity = 2", |e| num(e, "quantity") == 2.0, wide),
        ],
        exact: true,
        triples: 3,
        ..S::default()
    });
}

/// Values with a unit: what a read comes to and what `empty` writes are at the same scale.
#[test]
fn values_with_a_unit() {
    let wide = (0.0, 10000.0);
    let all_lines: fn(&El) -> bool = |_| true;
    hold(S {
        tag: "units",
        file: Some(("order.json", orders_json(r#","maxItems":2"#, r#""weight_g":{"type":"integer","enum":[500,1500]}"#, r#""weight_g""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("weight_g", nums(&[500.0, 1500.0]))], least: 0, most: 2, capped: true }],
        reads: vec![
            sum("total", "order.lines", "weight_g", "", all_lines, wide).typed("mass[g]", "g"),
            max("heaviest", "order.lines", "weight_g", "", all_lines, 0.0, wide).typed("mass[g]", "g"),
            max("floor", "order.lines", "weight_g", "", all_lines, 1000.0, wide).typed("mass[g]", "g"),
            min("lightest", "order.lines", "weight_g", "", all_lines, 2000.0, wide).typed("mass[g]", "g"),
        ],
        exact: true,
        triples: 1,
        ..S::default()
    });
}

/// Money read from a whole number of a currency, the corpus's form.
#[test]
fn money() {
    let wide = (0.0, 100.0);
    let all_lines: fn(&El) -> bool = |_| true;
    hold(S {
        tag: "money",
        file: Some(("order.json", orders_json(r#","maxItems":2"#, r#""amount_usd":{"type":"integer","minimum":1,"maximum":3},"fragile":{"type":"boolean"}"#, r#""amount_usd","fragile""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("amount_usd", nums(&[1.0, 2.0, 3.0])), closed("fragile", truths())], least: 0, most: 2, capped: true }],
        reads: vec![
            sum("total", "order.lines", "amount_usd", "", all_lines, wide).typed("money[USD]", "USD"),
            max("top_fragile", "order.lines", "amount_usd", "fragile = true", |e| yes(e, "fragile"), 0.0, wide).typed("money[USD]", "USD"),
            min("low", "order.lines", "amount_usd", "", all_lines, 100.0, wide).typed("money[USD]", "USD"),
        ],
        exact: true,
        triples: 1,
        ..S::default()
    });
}

/// Numbers that are not whole, in a `where` over a field of the contract.
#[test]
fn numbers_that_are_not_whole() {
    let wide = (0.0, 9.0);
    hold(S {
        tag: "fractions",
        file: Some(("order.json", orders_json(r#","maxItems":2"#, r#""price":{"type":"number","minimum":0.5,"maximum":2}"#, r#""price""#))),
        head: JSON_SHAPE.to_string(),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![sample("price", nums(&[0.5, 1.0, 1.5, 2.0]))], least: 0, most: 2, capped: true }],
        reads: vec![
            any("dear", "order.lines", "price > 1.5", |e| num(e, "price") > 1.5),
            all("cheap", "order.lines", "price <= 1", |e| num(e, "price") <= 1.0),
            count("dear_lines", "order.lines", "price > 1.5", |e| num(e, "price") > 1.5, wide),
            count("half", "order.lines", "price = 0.5", |e| num(e, "price") == 0.5, wide),
            count("size", "order.lines", "", |_| true, wide),
        ],
        exact: false,
        triples: 1,
        ..S::default()
    });
}

/// A column a table works out for each element: nothing here reads the table, so the check may not
/// say "never" of the count over it unless something else rules the combination out.
#[test]
fn a_column_a_table_works_out() {
    hold(S {
        tag: "computed",
        head: "elements lines\n  weight : number  range >=1 <=3\n\ntable big\npolicy unique\n| weight | -> is_big : bool |\n| >=2    | true            |\n| <2     | false           |\n".to_string(),
        colls: vec![Coll { key: "lines", fields: vec![closed("weight", nums(&[1.0, 2.0, 3.0]))], least: 0, most: 3, capped: true }],
        reads: vec![
            over_count("bigs", "lines", "is_big", |e| num(e, "weight") >= 2.0, (0.0, 3.0)),
            over_sum("total", "lines", "weight", (0.0, 99.0)),
            over_max("heaviest", "lines", "weight", "", |_| true, 0.0),
        ],
        exact: false,
        triples: 1,
        ..S::default()
    });
}

/// A sequence taken from the contract, summed beside what `from` reads of the same collection.
#[test]
fn a_sequence_from_the_contract_beside_the_reads() {
    let wide = (0.0, 3.0);
    hold(S {
        tag: "sequence-from",
        file: Some(("order.json", orders_json(r#","maxItems":3"#, &format!(r#"{CATEGORY},"quantity":{{"type":"integer","minimum":1,"maximum":2}}"#), r#""category","quantity""#))),
        head: format!("{JSON_SHAPE}\n\nenum kind = tool | part | service\n\nelements lines from order.lines\n  category : kind                  from category\n  quantity : number  range >=1 <=2  from quantity\n"),
        root: "order",
        colls: vec![Coll { key: "lines", fields: vec![closed("category", words(&["tool", "part", "service"])), closed("quantity", nums(&[1.0, 2.0]))], least: 0, most: 3, capped: true }],
        reads: vec![
            any("has_tool", "order.lines", "category = tool", |e| word(e, "category") == "tool"),
            over_count("tools", "lines", "category = tool", |e| word(e, "category") == "tool", wide),
            count("parts", "order.lines", "category = part", |e| word(e, "category") == "part", wide),
            over_sum("total", "lines", "quantity", (0.0, 99.0)),
        ],
        exact: false,
        no_read_back: true,
        triples: 1,
        ..S::default()
    });
}

// ── The forms that started this, one by one ────────────────────────────────────────────────

fn order_schema() -> String {
    orders_json(
        r#","maxItems":20"#,
        &format!(r#"{CATEGORY},"quantity":{{"type":"integer","minimum":1,"maximum":99}}"#),
        r#""category","quantity""#,
    )
}

/// A rule with the order's contract beside it, and what `check` says of it in a language.
fn checked(tag: &str, rule: &str, lang: Lang) -> (TempDir, String, Vec<Diag>) {
    let tmp = TempDir::new(&format!("walks-{tag}"));
    std::fs::write(tmp.path().join("order.json"), order_schema()).unwrap();
    let path = tmp.path().join("t.rule").to_string_lossy().into_owned();
    let ds = i18n::with(lang, || rulec::check_source(rule, &path));
    (tmp, path, ds)
}

fn one<'d>(ds: &'d [Diag], code: &str) -> &'d Diag {
    let found: Vec<&Diag> = ds.iter().filter(|d| d.code == code).collect();
    assert_eq!(found.len(), 1, "{code}: {ds:#?}");
    found[0]
}

const WRONG_COUNTS: &str = r##"rule t(t) v1

shape order = jsonschema "order.json" "#/$defs/Order"

inputs
  tools     : number  range >=0 <=20  from count order.lines where category = tool
  non_parts : number  range >=0 <=20  from count order.lines where category = not: part

constraint non_parts <= tools

outputs
  fee : money[USD]  round up(1USD)

table tool_fee
policy unique
| tools | non_parts | -> fee : money[USD] |
| >=1   | -         | 5USD                |
| 0     | 0         | 0USD                |
"##;

const WRONG_COUNTS_JA: &str = r##"rule 手数料(t) v1

shape 注文(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  工具の数(tools)      : number  range >=0 <=20  from count 注文.lines where category = tool
  部品以外(non_parts)  : number  range >=0 <=20  from count 注文.lines where category = not: part

constraint 部品以外 <= 工具の数

outputs
  料金(fee) : money[USD]  round up(1USD)

table 工具料(tool_fee)
policy unique
| 工具の数 | 部品以外 | -> 料金 : money[USD] |
| >=1      | -        | 5USD                 |
| 0        | 0        | 0USD                 |
"##;

/// The wrong direction, the form this started from: the same message as plain fields, with what
/// the collection holds in the example, and no fix on the contract's side, since a `.proto` could
/// only say it with a macro the check does not read.
#[test]
fn a_wrong_constraint_over_two_counts_is_e123_with_the_elements_that_break_it() {
    let (_t, _, ds) = checked("wrong", WRONG_COUNTS, Lang::En);
    let d = one(&ds, "E123");
    assert_eq!(d.title, "The contract lets `order.lines` through with values that break `constraint non_parts <= tools`");
    let said = d.notes.join("\n");
    assert!(
        said.contains("A request with `order.lines` holding 1 element with `category` = \"service\", for one, passes the contract's validation and gives `non_parts` = 1 and `tools` = 0, which breaks the `constraint`."),
        "{said}"
    );
    assert!(matches!(d.fix.kind, FixKind::None), "{:?}", d.fix);
    assert!(d.fix.text.is_none(), "{:?}", d.fix);
    let (_t, _, ds) = checked("wrong-ja", WRONG_COUNTS_JA, Lang::Ja);
    let d = one(&ds, "E123");
    assert_eq!(d.title, "契約は `注文.lines` について、`constraint 部品以外 <= 工具の数` を破る値も通します");
    let said = d.notes.join("\n");
    assert!(said.contains("`注文.lines` に `category` が \"service\" の要素が 1 件のリクエスト"), "{said}");
}

/// The right direction passes: a `category` that is `tool` is not a `part`.
#[test]
fn the_right_constraint_over_two_counts_passes() {
    let right = WRONG_COUNTS.replace("constraint non_parts <= tools", "constraint tools <= non_parts").replace("| 0     | 0         |", "| 0     | -         |");
    let (_t, _, ds) = checked("right", &right, Lang::En);
    assert!(!ds.iter().any(|d| d.severity == rulec::diag::Severity::Error), "{ds:#?}");
    let right = WRONG_COUNTS_JA.replace("constraint 部品以外 <= 工具の数", "constraint 工具の数 <= 部品以外").replace("| 0        | 0        |", "| 0        | -        |");
    let (_t, _, ds) = checked("right-ja", &right, Lang::Ja);
    assert!(!ds.iter().any(|d| d.severity == rulec::diag::Severity::Error), "{ds:#?}");
}

const GAP: &str = r##"rule t(t) v1

shape order = jsonschema "order.json" "#/$defs/Order"

inputs
  has_tool  : bool  from any order.lines where category = tool
  all_parts : bool  from all order.lines where category = part

outputs
  fee : money[USD]  round up(1USD)

table tool_fee
policy unique
| has_tool | all_parts | -> fee : money[USD] |
| true     | false     | 5USD                |
| false    | true      | 0USD                |
| false    | false     | 2USD                |
"##;

/// E101 stays, because a caller who works the values out by hand can send them, and says so.
#[test]
fn a_gap_no_definition_reaches_keeps_e101_and_says_so() {
    let (_t, _, ds) = checked("gap", GAP, Lang::En);
    let said = one(&ds, "E101").notes.join("\n");
    for line in [
        "An input that matches no row: has_tool = true, all_parts = true",
        "This combination never comes out of the definitions (`has_tool` is `any order.lines where category = tool`, `all_parts` is `all order.lines where category = part`).",
        "A row is still asked for because a call that does not read through `from` reaches it: the code generated for NumPy, Rust, Go, Swift, Java, SQL and Wasm does not read `from`, and takes whatever the caller worked out.",
    ] {
        assert!(said.contains(line), "{said}");
    }
    let (_t, _, ds) = checked("gap-ja", GAP, Lang::Ja);
    let said = one(&ds, "E101").notes.join("\n");
    assert!(said.contains("この組み合わせは、定義からは起こりません"), "{said}");
    assert!(said.contains("`from` を読まない呼び出しには届く"), "{said}");
}

/// Over a sequence the reason differs: no target takes a summary from its caller.
#[test]
fn a_gap_over_a_sequence_says_the_check_reads_a_summary_as_a_column() {
    let rule = "rule t(t) v1\n\nelements lines\n  is_tool : bool\n  weight  : number  range >=1 <=100\n\ncount tools over lines where is_tool  range >=0 <=5\nsum total over lines of weight  range >=0 <=1000\nmax heaviest over lines of weight empty 0\n\noutputs\n  fee : money[USD]  round up(1USD)\n\ntable fee_of\npolicy unique\n| tools | total | heaviest | -> fee : money[USD] |\n| 0     | 0     | 0        | 0USD                |\n| >=1   | >=1   | >=1      | 5USD                |\n| 0     | >=1   | >=1      | 2USD                |\n";
    let (_t, _, ds) = checked("gap-seq", rule, Lang::En);
    let said = one(&ds, "E101").notes.join("\n");
    assert!(said.contains("An input that matches no row: tools = 0, total = 0, heaviest = 1"), "{said}");
    assert!(said.contains("(`tools` is `count over lines where is_tool`, `total` is `sum over lines of weight`, `heaviest` is `max over lines of weight empty 0`)"), "{said}");
    assert!(said.contains("The check of a table reads a summary as one column"), "{said}");
    assert!(!said.contains("does not read `from`"), "{said}");
}

/// A gap part of which occurs: the example is put on what occurs, and nothing says "never".
#[test]
fn a_gap_part_of_which_occurs_shows_what_occurs() {
    let rule = r##"rule t(t) v1

shape order = jsonschema "order.json" "#/$defs/Order"

inputs
  has_tool : bool                     from any order.lines where category = tool
  tools    : number  range >=0 <=20   from count order.lines where category = tool

outputs
  fee : money[USD]  round up(1USD)

table tool_fee
policy unique
| has_tool | tools | -> fee : money[USD] |
| true     | >3    | 5USD                |
| false    | -     | 2USD                |
"##;
    let (_t, _, ds) = checked("gap-part", rule, Lang::En);
    let said = one(&ds, "E101").notes.join("\n");
    assert!(!said.contains("never comes out"), "{said}");
    assert!(!said.contains("tools = 0"), "tools = 0 with a tool never occurs: {said}");
    assert!(said.contains("An input that matches no row: has_tool = true, tools = 1"), "{said}");
}

const ROW: &str = r##"rule t(t) v1

shape order = jsonschema "order.json" "#/$defs/Order"

inputs
  has_tool  : bool  from any order.lines where category = tool
  all_parts : bool  from all order.lines where category = part

outputs
  fee : money[USD]  round up(1USD)

table tool_fee
policy unique
| has_tool | all_parts | -> fee : money[USD] |
| true     | true      | 99USD               |
| true     | false     | 5USD                |
| false    | true      | 0USD                |
| false    | false     | 2USD                |
"##;

/// The row of a price that no order will ever pay warns, and the rule still passes, because the
/// table needs it to be complete.
#[test]
fn a_row_no_definition_reaches_warns() {
    let (_t, _, ds) = checked("row", ROW, Lang::En);
    let d = one(&ds, "W133");
    assert_eq!(d.title, "row 1 is reached only by a combination the definitions never give");
    assert_eq!((d.table.as_deref(), d.row), (Some("tool_fee"), Some(1)));
    assert!(!ds.iter().any(|d| d.severity == rulec::diag::Severity::Error), "{ds:#?}");
    let (_t, _, ds) = checked("row-ja", ROW, Lang::Ja);
    assert_eq!(one(&ds, "W133").title, "行1 は、定義からは起こらない組み合わせでしか当たりません");
}

/// A boolean `constraint` is still what it was.
#[test]
fn a_constraint_over_truth_values_is_still_e018() {
    let rule = GAP.replace("outputs\n", "constraint has_tool <= all_parts\n\noutputs\n");
    let (_t, _, ds) = checked("e018", &rule, Lang::En);
    one(&ds, "E018");
    assert!(!ds.iter().any(|d| d.code == "E123"), "{ds:#?}");
}

/// Dates: `max` and `min` read them as day numbers, and the example shows dates.
#[test]
fn a_constraint_over_the_dates_of_a_collection() {
    let contract = orders_json(r#","maxItems":5"#, r#""shipped":{"type":"string","format":"date"}"#, r#""shipped""#);
    let rule = |empties: (&str, &str), k: &str| {
        format!(
            "rule t(t) v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  latest   : date  from max order.lines of shipped empty {}\n  earliest : date  from min order.lines of shipped empty {}\n\nconstraint {k}\n\noutputs\n  x(x) : number  round down(1)\n\ntable k\npolicy first\n| latest | -> x : number |\n| -      | 1             |\n",
            empties.0, empties.1
        )
    };
    let tmp = TempDir::new("walks-dates");
    std::fs::write(tmp.path().join("order.json"), contract).unwrap();
    let path = tmp.path().join("t.rule").to_string_lossy().into_owned();
    let e123 = |src: &str| diags(src, &path).into_iter().filter(|d| d.code == "E123").map(|d| d.notes.join("\n")).collect::<Vec<_>>();
    // the smallest of some dates is never after the largest, and with one `empty` nor on no dates
    assert!(e123(&rule(("2026-01-01", "2026-01-01"), "earliest <= latest")).is_empty());
    // with two, no dates at all breaks it
    let found = e123(&rule(("2026-01-01", "2026-12-31"), "earliest <= latest"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("an empty `order.lines`") && found[0].contains("`earliest` = 2026-12-31"), "{found:?}");
    // and the other way two dates do
    let found = e123(&rule(("2026-01-01", "2026-01-01"), "latest <= earliest"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("`order.lines` holding"), "{found:?}");
}

const REFUSED: &str = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("t", sys.argv[1])
m = importlib.util.module_from_spec(spec)
sys.modules["t"] = m
spec.loader.exec_module(m)
for order in json.load(open(sys.argv[2], encoding="utf-8")):
    try:
        print("answer", m.t_from(order))
    except m.RuleInputError as e:
        print("refused", e)
"#;

/// What E123 says happens: the request of its example passes the contract and is refused by the
/// code generated for the rule, and one that keeps the constraint is not.
#[test]
fn the_example_of_e123_is_refused_at_run_time() {
    if !ready(Need::Python, have_python, "python3 is not there") {
        return;
    }
    let (tmp, path, ds) = checked("run", WRONG_COUNTS, Lang::En);
    let said = one(&ds, "E123").notes.join("\n");
    assert!(said.contains("\"service\""), "{said}");
    let (f, c) = rulec::prepare(WRONG_COUNTS, &path).unwrap();
    let code = rulec::codegen::Gen::new(&f, &c, WRONG_COUNTS, &path).python();
    let line = |c: &str| format!(r#"{{"category":"{c}","quantity":1}}"#);
    let data = format!("[{{\"lines\":[{}]}},{{\"lines\":[{}]}},{{\"lines\":[]}}]", line("service"), line("tool"));
    let got = python(tmp.path(), REFUSED, &code, &data);
    assert!(got[0].starts_with("refused"), "{got:?}");
    assert_eq!(got[1], "answer 5", "{got:?}");
    assert_eq!(got[2], "answer 0", "{got:?}");
}

const REACHED: &str = r#"
import importlib.util, itertools, json, sys
spec = importlib.util.spec_from_file_location("t", sys.argv[1])
m = importlib.util.module_from_spec(spec)
sys.modules["t"] = m
spec.loader.exec_module(m)
fees = set()
for size in range(4):
    for lines in itertools.combinations_with_replacement(["tool", "part", "service"], size):
        fees.add(m.t_from({"lines": [{"category": c, "quantity": 1} for c in lines]}))
print("through from", sorted(fees))
print("by hand", m.t(True, True))
"#;

/// What W133 says of the row: no order reaches it through `from`, and a caller who works the values
/// out by hand does.
#[test]
fn the_row_of_w133_is_reached_only_by_hand() {
    if !ready(Need::Python, have_python, "python3 is not there") {
        return;
    }
    let (tmp, path, ds) = checked("reach", ROW, Lang::En);
    one(&ds, "W133");
    let (f, c) = rulec::prepare(ROW, &path).unwrap();
    let code = rulec::codegen::Gen::new(&f, &c, ROW, &path).python();
    let got = python(tmp.path(), REACHED, &code, "[]");
    assert_eq!(got, ["through from [0, 2, 5]", "by hand 99"], "{got:?}");
}

/// A collection too finely cut to be read is left alone. `sum` and `max` keep the stretches of a
/// field apart, and 300 values with holes between them are 300 stretches, past what is read: the
/// answers are the same as before this module, nothing said on the row no request reaches and nothing
/// added to the gap. The same rule over a few values says both, so it is the cutting that keeps quiet.
#[test]
fn what_cannot_be_read_stays_quiet() {
    let answers = |values: usize| {
        let values: Vec<String> = (0..values).map(|i| (i * 2).to_string()).collect();
        let contract = orders_json(r#","maxItems":3"#, &format!(r#""quantity":{{"type":"integer","enum":[{}]}}"#, values.join(",")), r#""quantity""#);
        let head = "rule t(t) v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  has_two   : bool  from any order.lines where quantity = 2\n  all_four  : bool  from all order.lines where quantity = 4\n  total     : number  range >=0 <=1794  from sum order.lines of quantity\n\noutputs\n  x(x) : number  round down(1)\n\ntable k\npolicy unique\n| has_two | all_four | total | -> x : number |\n";
        let tmp = TempDir::new("walks-quiet");
        std::fs::write(tmp.path().join("order.json"), contract).unwrap();
        let path = tmp.path().join("t.rule").to_string_lossy().into_owned();
        let whole = format!("{head}| true | true | - | 99 |\n| true | false | - | 1 |\n| false | true | - | 2 |\n| false | false | - | 3 |\n");
        let ds = diags(&whole, &path);
        assert!(ds.iter().all(|d| d.severity != rulec::diag::Severity::Error), "{ds:#?}");
        let warned = ds.iter().any(|d| d.code == "W133");
        let gap = format!("{head}| true | false | - | 1 |\n| false | true | - | 2 |\n| false | false | - | 3 |\n");
        let noted = one(&diags(&gap, &path), "E101").notes.join("\n").contains("never comes out");
        (warned, noted)
    };
    assert_eq!(answers(300), (false, false));
    assert_eq!(answers(4), (true, true));
}

