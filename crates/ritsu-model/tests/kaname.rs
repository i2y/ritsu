//! kaname's reference interpreter and its analysis of ranges, held to `KanameCore` (DESIGN 11.3;
//! kaname's DESIGN 15.3).
//!
//! Every module of kaname's examples and of its tests (`tests/modules/`), and modules made from the
//! examples whose checks across the borders come to each verdict (`materials`), is checked as `ritsu
//! kaname` checks it, every language joined and ritsu-cross's verdicts handed over, and written in
//! the form the model reads: each expression with what kaname's check decided of it — where a number
//! is read in another unit and by what factor, the step its type says, the place each argument goes
//! to, what a rule's outputs and a date's days can be — and the verdicts as kaname took them, each at
//! its place. Then the model is asked two things, and its answers compared with kaname's:
//!
//! - the analysis of the module: for every function, what it answers, every place where it can fail
//!   with the values given there and whether they are shown (or wait for a verdict), its effect, and
//!   the relations each of its calls of a rule keeps between the inputs given; and what every
//!   expression can be;
//! - every vector of every function (`kaname::vectors::choose`), run by the reference interpreter
//!   and by the model: the answer, or the failure's kind and line, or that it did not stop. The
//!   rules and the dates answer the model what rulec and koyomi answered the interpreter on the same
//!   run (the calls of its trace).
//!
//! Every value the interpreter gives an expression on those runs is also held to what the analysis
//! says of it. `-- --nocapture` shows a `compared kaname <module>: …` line for each module.

use kaname::ast::*;
use kaname::check::{Checked, Run};
use kaname::interp::{Interp, Stop, Watch};
use kaname::names::{column_slot, column_values, date_input_slot};
use kaname::ranges::{Abs, Analyzer, Iv, Outcome, Rel, What};
use kaname::suite::Suite;
use kaname::ty::*;
use kaname::types::Ref;
use kaname::value::{Program, Value};
use ritsu_base::json::Json as RJson;
use ritsu_model::{compare, program};
use ritsu_ports::Found;
use ritsu_testkit::{Need, TempDir, ready};
use ritsu_units::{Dim, Rat};
use serde_json::{Value as J, json};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;

fn kaname_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../kaname")
}

/// Every language joined, and what ritsu-cross's checks across the borders come to at a module's
/// calls, as `ritsu kaname` hands them over (ritsu's `languages.rs`). A module checked from a text and
/// no file has no verdicts: its places across the borders wait for them.
fn suite() -> Suite {
    let j = ritsu_project::Joined::new();
    let verdicts = Rc::new(ritsu_cross::ProgramVerdicts::new(&j));
    let s: Suite = j.ports().into();
    s.with_verdicts(verdicts)
}

/// Modules made from the examples, each laid out in a directory of its own, whose checks across the
/// borders come to each verdict (the cases of `crates/ritsu-cross/tests/programs.rs`): an amount a run
/// takes below 0 (an example) and one only the ranges take below 0 (undecided), a precondition no
/// relation keeps (undecided) and one a run breaks (an example), a koyomi date given a day past its
/// input's range (an example), a koyomi day given to a rule whose range starts later (an example) and
/// narrowed to it (shown), and a rule whose date input's range is a koyomi date (X2's days: kept, and
/// given the days of another date). And a module whose functions spread a refund over the running
/// totals of a list, each after an `if` that leaves out a refund above a sum (kaname's DESIGN 32,
/// 33): the sum of the same list by the same field keeps the rule's precondition `amount <= total`
/// (written as functions, and after the list with the lambdas' parameters named apart); a sum of
/// another list, by another field or by more than a field, keeps nothing, and neither do running
/// totals a `let` names.
fn materials(tmp: &TempDir) -> Vec<(String, PathBuf)> {
    let examples = kaname_dir().join("examples");
    let changed = |name: &str, tag: &str, edits: &[(&str, &str)]| -> (String, PathBuf) {
        let d = tmp.path().join(format!("{name}-{tag}"));
        ritsu_testkit::tmp::copy_dir(&examples.join(name), &d);
        let kn = d.join(format!("{name}.kn"));
        let mut src = std::fs::read_to_string(&kn).unwrap();
        for (from, to) in edits {
            assert!(src.contains(from), "{name} has no {from:?}");
            src = src.replacen(from, to, 1);
        }
        // the example's `examples` are its own: a changed module drops them
        src = src.split("\nexamples\n").next().unwrap().to_string() + "\n";
        std::fs::write(&kn, src).unwrap();
        (format!("{name} ({tag})"), kn)
    };
    let net = ("amount: payment.amount),", "amount: payment.amount - fee),");
    let mut out = vec![
        changed("payout", "net", &[net]),
        changed("payout", "net of a dollar", &[net, ("range >=1USDc <=100_000_000USDc", "range >=100USDc <=100_000_000USDc")]),
        changed("discount_split", "reduce", &[("upto: r.upto", "upto: reduce([r.item.amount], r.before, (acc, x) => acc + x)")]),
        changed("discount_split", "swapped", &[("before: r.before, upto: r.upto", "before: r.upto, upto: r.before")]),
        changed("late_payment", "paid on", &[("terms.due(invoiced: invoice.invoiced)", "terms.due(invoiced: paid_on)")]),
    ];
    let terms = "dates payment_terms v1\ndescription \"Closes on the 20th and pays on the 10th of the next month\"\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-20\n\ndate closing = received\n  close day 20\n\ndate payment = closing\n  day 10 of month +1\n";
    let batch = "rule batch v1\ndescription \"The billing batch a payment day falls in\"\n\nenum run = spring | autumn\n\ninputs\n  pay_day : date  range RANGE\n\noutputs\n  batch : run\n\ntable pick\npolicy unique\n| pay_day      | -> batch : run |\n| <=2026-08-31 | spring         |\n| >=2026-09-01 | autumn         |\n";
    let pick = "module billing v1\n\nuse dates terms from \"payment_terms.cal\"\nuse rule batch from \"batch.rule\"\n\nfn pick(received: date range >=2026-01-01 <=2026-12-20) -> batch.run\n  batch(pay_day: terms.DATE(received: received)).batch\n";
    let narrowed = "module billing v1\n\nuse dates terms from \"payment_terms.cal\"\nuse rule batch from \"batch.rule\"\n\nfn pick(received: date range >=2026-01-01 <=2026-12-20) -> batch.run?\n  let due = terms.payment(received: received)\n  if due >= 2026-03-01\n    batch(pay_day: due).batch\n  else\n    none\n";
    let mut billing = |tag: &str, range: &str, kn: &str, date: &str| {
        let d = tmp.path().join(format!("billing-{tag}"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("payment_terms.cal"), terms).unwrap();
        std::fs::write(d.join("batch.rule"), batch.replace("RANGE", range)).unwrap();
        let f = d.join("billing.kn");
        std::fs::write(&f, kn.replace("DATE", date)).unwrap();
        out.push((format!("billing ({tag})"), f));
    };
    billing("late", ">=2026-03-01 <=2027-01-31", pick, "payment");
    billing("narrowed", ">=2026-03-01 <=2027-01-31", narrowed, "payment");
    billing("koyomi's days", "from koyomi \"payment_terms.cal\" date payment", pick, "payment");
    billing("another date's days", "from koyomi \"payment_terms.cal\" date payment", pick, "closing");
    let share = "rule refund_share v1\ndescription \"One line's share of a refund spread over the lines in proportion to what is left on each\"\n\ninputs\n  amount : money[USDc]  range >=1USDc <=1_000_000USDc\n  before : money[USDc]  range >=0USDc <=100_000_000USDc\n  upto   : money[USDc]  range >=0USDc <=100_000_000USDc\n  total  : money[USDc]  range >=1USDc <=100_000_000USDc\n\nconstraint before <= upto\nconstraint upto <= total\nconstraint amount <= total\n\noutputs\n  share : money[USDc]  round down(1USDc)\n\nderive to_before : money[USDc] = allocate(amount, before, total)  range >=0USDc <=1_000_000USDc\nderive to_upto   : money[USDc] = allocate(amount, upto, total)    range >=0USDc <=1_000_000USDc\n\nresult share = to_upto - to_before\n";
    let to_rule = "refund_share(amount: amount, before: r.before, upto: r.upto, total: r.total).share";
    let split = |name: &str, count: &str, guard: &str, walk: &str| {
        format!(
            "fn {name}(lines: list[Line] count >=1 <={count}, others: list[Line] count >=1 <=10, amount: money[USDc] range >=1USDc <=1_000_000USDc) -> list[money[USDc]]?\n  if amount > {guard}\n    none\n  else\n    {}\n",
            walk.replace("RULE", to_rule)
        )
    };
    let refunds = [
        "module refunds v1\n\nuse rule refund_share from \"share.rule\"\n\nrecord Line\n  left  : money[USDc]  range >=1USDc <=10_000_000USDc\n  other : money[USDc]  range >=1USDc <=10_000_000USDc\n".to_string(),
        split("same", "10", "sum(lines, l => l.left)", "map(running_totals(lines, l => l.left), r => RULE)"),
        split("apart", "10", "lines.sum(x => x.left)", "lines.running_totals(y => y.left).map(r => RULE)"),
        split("another_list", "10", "sum(others, l => l.left)", "map(running_totals(lines, l => l.left), r => RULE)"),
        split("another_field", "10", "sum(lines, l => l.other)", "map(running_totals(lines, l => l.left), r => RULE)"),
        split("more_than_a_field", "5", "sum(lines, l => l.left * 2)", "map(running_totals(lines, l => l.left * 2), r => RULE)"),
        split("named", "10", "sum(lines, l => l.left)", "let rs = running_totals(lines, l => l.left)\n    map(rs, r => RULE)"),
    ];
    let d = tmp.path().join("refunds");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("share.rule"), share).unwrap();
    let f = d.join("refunds.kn");
    std::fs::write(&f, refunds.join("\n")).unwrap();
    out.push(("refunds".to_string(), f));
    out
}

/// The modules: the examples (English and Japanese) and the test modules of kaname's
/// `tests/modules/`, which read the examples' rules by their paths from the crate (checked as the
/// file `t.kn` beside it). `corners` holds what the others do not reach (a rounding of a half
/// exactly, a call of a function that goes round from another ring, a `reduce` whose running value
/// settles on its third round, a difference of two names known to be equal), `bounds` the places
/// where `ranges.rs` once was not an estimate from above, `lift` (English and Japanese) a rule's
/// input of an enum held to a range over its values, `open` (English and Japanese) values whose
/// ranges are not written, each the whole of its type (kaname's DESIGN 22), `sums` (English and
/// Japanese) the values of enums that have fields, the `match`es that name them, `else` and literals,
/// records made from another and values compared whole (kaname's DESIGN 23), `temps` (English and
/// Japanese) temperatures read in the other unit with that unit's zero, and constants below zero of
/// every unit (kaname's DESIGN 3.10), `dicts` and `lists2` (English and Japanese) dicts, sets and
/// the builtins of lists, written after a value (kaname's DESIGN 24), and `texts` (English and
/// Japanese) strings as their code points, counted, cut, found, read and compared (kaname's DESIGN
/// 25), `generic` and `results` (English and Japanese) generic functions, records and enums read
/// again where they are called, results, and `try` (kaname's DESIGN 26), `glib`, the generic
/// functions `gcalls` calls, and `hof` (English and Japanese) functions that take functions,
/// lambdas made values and called, and what is `private` (kaname's DESIGN 27, 28.2). `calls` and
/// `gcalls` are left out: they call the functions of another module, and the model reads one module
/// (kaname's DESIGN 15.4).
const EXAMPLES: [&str; 16] = [
    "examples/order_flow/order_flow.kn",
    "examples/order_flow/order_flow.ja.kn",
    "examples/stock_diff/stock_diff.kn",
    "examples/stock_diff/stock_diff.ja.kn",
    "examples/handles/handles.kn",
    "examples/handles/handles.ja.kn",
    "examples/catalog/catalog.kn",
    "examples/catalog/catalog.ja.kn",
    "examples/payout/payout.kn",
    "examples/payout/payout.ja.kn",
    "examples/late_payment/late_payment.kn",
    "examples/late_payment/late_payment.ja.kn",
    "examples/media_mail/media_mail.kn",
    "examples/media_mail/media_mail.ja.kn",
    "examples/discount_split/discount_split.kn",
    "examples/discount_split/discount_split.ja.kn",
];
const MODULES: [&str; 34] = [
    "calc", "places", "lists", "inner", "rec", "ranges", "walks", "names", "lib", "corners", "bounds", "lift", "lift.ja", "open", "open.ja", "sums", "sums.ja", "temps", "temps.ja", "dicts",
    "dicts.ja", "lists2", "lists2.ja", "texts", "texts.ja", "generic", "generic.ja", "results", "results.ja", "glib", "hof", "hof.ja", "trees", "trees.ja",
];

// ── Types, as kaname's analysis reads them ──────────────────────────────────

fn strip(t: &Ty) -> &Ty {
    match t {
        Ty::Opt(x) | Ty::List(x) => strip(x),
        x => x,
    }
}

fn strip_opt(t: &Ty) -> &Ty {
    match t {
        Ty::Opt(x) => strip_opt(x),
        x => x,
    }
}

/// The step the values of a type land on: a constant's value, the type's step; a date's day.
fn step_of(t: &Ty) -> Option<Rat> {
    match strip(t) {
        Ty::Num(n) => match n.konst {
            Some(k) => Some(Rat { num: k.num.abs(), den: k.den }),
            None => n.step,
        },
        Ty::Date => Some(Rat::int(1)),
        _ => None,
    }
}

/// How a value of `from` is read in `to`: times a factor, plus the shift between two zeros.
fn factor_of(from: &Ty, to: &Ty) -> Option<(Rat, Rat)> {
    let (Ty::Num(a), Ty::Num(b)) = (strip(from), strip(to)) else { return None };
    let dimless = |n: &NumTy| matches!(n.unit.dim, Dim::Number | Dim::Rate);
    if dimless(a) && (dimless(b) || a.konst.is_some_and(|k| k.num == 0)) {
        return None;
    }
    let shift = if a.unit.compares_only() && b.unit.compares_only() { convert(Rat::zero(), &a.unit, &b.unit).unwrap_or(Rat::zero()) } else { Rat::zero() };
    match factor(&a.unit, &b.unit) {
        Some(f) if f != Rat::int(1) || shift.num != 0 => Some((f, shift)),
        _ => None,
    }
}

/// The type two numbers are read in to be compared: the one that is not a constant, else the first;
/// through the lists and the optionals that hold them (`interp::common`).
fn common<'t>(a: &'t Ty, b: &'t Ty) -> &'t Ty {
    match (strip(a), strip(b)) {
        (Ty::Num(x), Ty::Num(y)) if x.konst.is_some() && y.konst.is_none() => b,
        _ => a,
    }
}

fn has_unit(t: &Ty) -> bool {
    t.present().num().is_some_and(NumTy::has_unit)
}

// ── JSON of the model's form ────────────────────────────────────────────────

fn rat(r: Rat) -> J {
    if r.den == 1 { json!(r.num.to_string()) } else { json!(format!("{}/{}", r.num, r.den)) }
}

fn end(e: Option<Rat>) -> J {
    e.map(rat).unwrap_or(J::Null)
}

fn den(d: Option<i128>) -> J {
    d.map(|d| json!(u64::try_from(d).expect("a bound on a denominator past u64"))).unwrap_or(J::Null)
}

fn iv(i: &Iv) -> J {
    json!([end(i.lo), end(i.hi), den(i.den)])
}

fn range(r: &Option<Range>) -> J {
    r.as_ref().map(|r| json!([end(r.min), end(r.max)])).unwrap_or(J::Null)
}

fn rel(r: Rel) -> &'static str {
    match r {
        Rel::Lt => "lt",
        Rel::Le => "le",
        Rel::Eq => "eq",
    }
}

fn abs(a: &Abs) -> J {
    match a {
        Abs::Never => json!("never"),
        Abs::Any => json!("any"),
        Abs::Str(c) => json!({"str": iv(c)}),
        Abs::Num(i) => json!({"num": iv(i)}),
        Abs::Date(i) => json!({"date": iv(i)}),
        Abs::Set(None) => json!({"set": null}),
        Abs::Set(Some(s)) => json!({"set": s.iter().collect::<Vec<_>>()}),
        Abs::Opt(n, x) => json!({"opt": [n, abs(x)]}),
        Abs::List(c, x) => json!({"list": [iv(c), abs(x)]}),
        Abs::Record(r) => json!({"rec": [
            r.fields.iter().map(|(n, x)| json!([n, abs(x)])).collect::<Vec<_>>(),
            r.rels.iter().map(|(x, r, y)| json!([x, rel(*r), y])).collect::<Vec<_>>(),
        ]}),
        Abs::Transfer(args) => json!({"tr": args.iter().map(|(n, x)| json!([n, abs(x)])).collect::<Vec<_>>()}),
        // a value of an enum that has fields keeps no relation between its fields (kaname's DESIGN 23.7)
        Abs::Variants(vs) => json!({"variants": vs.iter().map(|(v, r)| {
            assert!(r.rels.is_empty(), "a value of an enum with relations between its fields: {v}");
            json!([v, r.fields.iter().map(|(n, x)| json!([n, abs(x)])).collect::<Vec<_>>()])
        }).collect::<Vec<_>>()}),
        Abs::Dict(c, k, x) => json!({"dict": [iv(c), abs(k), abs(x)]}),
        Abs::Members(c, x) => json!({"members": [iv(c), abs(x)]}),
        // a function value (kaname's DESIGN 27.4): the lambdas it can be, by their ids, each once from
        // the least (what each caught is the analysis's to follow, and is held to the model through the
        // values of the expressions of the lambdas' bodies); and what the analysis does not see of it
        Abs::Fns(ls, unseen) => {
            let mut ids: Vec<usize> = ls.iter().map(|l| l.expr).collect();
            ids.sort_unstable();
            ids.dedup();
            let u = match unseen {
                None => J::Null,
                Some(None) => json!("any"),
                Some(Some(e)) => json!([e.fails.iter().map(|k| k.word()).collect::<Vec<_>>(), e.diverges]),
            };
            json!({"fns": [ids, u]})
        }
    }
}

fn tid(id: &TransferId) -> String {
    format!("{}|{}|{}", id.book.display(), id.transfer, id.op)
}

fn val(v: &Value) -> J {
    match v {
        Value::Num(r) => json!({"n": rat(*r)}),
        Value::Bool(b) => json!({"b": b}),
        Value::Str(s) => json!({"s": s}),
        Value::Date(d) => json!({"d": d}),
        Value::Enum(x) => json!({"e": x}),
        Value::Record(fs) => json!({"r": fs.iter().map(|(n, x)| json!([n, val(x)])).collect::<Vec<_>>()}),
        Value::List(xs) => json!({"l": xs.iter().map(val).collect::<Vec<_>>()}),
        Value::None => J::Null,
        Value::Transfer(t) => json!({"t": [tid(&t.id), t.args.iter().map(|(n, x)| json!([n, val(x)])).collect::<Vec<_>>()]}),
        Value::Variant(v, fs) => json!({"v": [v, fs.iter().map(|(n, x)| json!([n, val(x)])).collect::<Vec<_>>()]}),
        Value::Dict(ps) => json!({"dict": ps.iter().map(|(k, x)| json!([val(k), val(x)])).collect::<Vec<_>>()}),
        Value::Set(xs) => json!({"set": xs.iter().map(val).collect::<Vec<_>>()}),
        // a function value is no input and no answer of a run (kaname's DESIGN 27.1)
        Value::Closure(c) => json!({"clo": c.expr}),
    }
}

/// The key `ranges.rs` keeps a place by, from what it is (a precondition by what it says).
fn key_of(w: &What) -> String {
    match w {
        What::RuleInput { input, .. } | What::RuleEnum { input, .. } => format!("input:{input}"),
        What::RuleField { list, field, .. } => format!("elem:{list}.{field}"),
        What::DateInt { input, .. } => format!("int:{input}"),
        What::Param { param, .. } => format!("param:{param}"),
        What::Answer { .. } => "answer".into(),
        What::Field { field, .. } => format!("field:{field}"),
        What::Isqrt => "isqrt".into(),
        What::Overflow => "overflow".into(),
        What::Zero => "zero".into(),
        What::Precondition { said, .. } => format!("pre:{said}"),
        What::RuleDate { input, .. } => format!("rdate:{input}"),
        What::Amount { param, .. } => format!("amount:{param}"),
        What::DateDay { input, .. } => format!("day:{input}"),
    }
}

/// The text of an expression on its line, as the interpreter's trace names a callee.
fn source(c: &Checked, sp: Span) -> String {
    let line = c.src.lines().nth(sp.line.saturating_sub(1)).unwrap_or("");
    let cs: Vec<char> = line.chars().collect();
    let (a, b) = (sp.col.saturating_sub(1).min(cs.len()), sp.end.saturating_sub(1).min(cs.len()));
    cs[a..b.max(a)].iter().collect::<String>().trim().to_string()
}

// ── The module in the model's form ──────────────────────────────────────────

struct Ex<'a> {
    c: &'a Checked,
    p: &'a Program,
    suite: &'a Suite,
    an: Analyzer<'a>,
    rules: RefCell<Vec<PathBuf>>,
    dates: RefCell<Vec<PathBuf>>,
    /// The calls of rules and dates, by their line and the callee as written (how the trace names
    /// them): the rule's index; the dates file's index and the date.
    rule_calls: RefCell<BTreeMap<(usize, String), usize>>,
    date_calls: RefCell<BTreeMap<(usize, String), (usize, String)>>,
    /// The names `ok x` and `err e` give in the arms written now (kaname's DESIGN 26.3), each with the
    /// name the model gives the result it takes and the field that holds the value: the model's
    /// pattern names the value an arm takes, and the arm reads the value it holds from it.
    payloads: RefCell<Vec<(String, String, &'static str)>>,
    /// The next id of an expression the model is given that the module does not have (the name of a
    /// result an arm takes), past every id of the module's.
    fresh: std::cell::Cell<usize>,
    /// The places of the records and the enums that hold themselves the module's places reach, each by
    /// the type its values are counted as (kaname's DESIGN 28.1): the model's `slots`.
    slots: RefCell<Vec<(Ty, J)>>,
}

impl<'a> Ex<'a> {
    fn new(c: &'a Checked, p: &'a Program, suite: &'a Suite) -> Ex<'a> {
        Ex {
            c,
            p,
            suite,
            an: Analyzer::new(c, p, suite),
            rules: RefCell::default(),
            dates: RefCell::default(),
            rule_calls: RefCell::default(),
            date_calls: RefCell::default(),
            payloads: RefCell::default(),
            fresh: std::cell::Cell::new(c.module.exprs),
            slots: RefCell::default(),
        }
    }

    fn ty(&self, e: &Expr) -> &'a Ty {
        self.c.typed.types.get(e.id).unwrap_or(&Ty::Error)
    }

    fn rf(&self, e: &Expr) -> &'a Ref {
        self.c.typed.refs.get(e.id).unwrap_or(&Ref::None)
    }

    /// A value of `from` read in `to`, where the interpreter reads it so, failing at `at`.
    fn conv(&self, from: &Ty, to: &Ty, at: &Expr) -> J {
        match factor_of(from, to) {
            Some((f, shift)) => json!([rat(f), rat(shift), at.id, at.span.line]),
            None => J::Null,
        }
    }

    /// What a place declares. `reached`: a parameter's, where an end not written is the type's (all
    /// of int64 for a number, 0001-01-01 to 9999-12-31 for a date, 0 to 2⁶³ − 1 elements for a list;
    /// kaname's DESIGN 22.1, 22.6), as the interpreter holds an argument to it and the analysis a value
    /// given to it.
    fn slot(&self, s: &Slot, reached: bool) -> J {
        self.slot_in(s, reached, false)
    }

    /// The number of a record or an enum that holds itself in the module's table (`slots`, kaname's
    /// DESIGN 28.1), its place written there once, as a parameter reaches it; None for any other type.
    fn table(&self, t: &Ty) -> Option<usize> {
        let k = self.p.nest_key(t)?;
        if let Some(i) = self.slots.borrow().iter().position(|(x, _)| *x == k) {
            return Some(i);
        }
        let i = self.slots.borrow().len();
        self.slots.borrow_mut().push((k.clone(), J::Null));
        let j = self.slot_in(&Slot::of(k), true, true);
        self.slots.borrow_mut()[i].1 = j;
        Some(i)
    }

    /// What a place declares; `own`: the place of a record or an enum that holds itself in the table,
    /// the type itself written out, the values of such types inside it by their numbers.
    fn slot_in(&self, s: &Slot, reached: bool, own: bool) -> J {
        if !own && let Some(j) = self.nested_slot(s, reached) {
            return j;
        }
        let t = strip_opt(&s.ty);
        // a list's elements, a set's members, a dict's values: what the count counts and the range
        // holds; a dict's keys, each its own place (kaname's DESIGN 24.4)
        let (list, elem, keys) = match t {
            Ty::List(x) | Ty::Set(x) => (true, strip_opt(x), None),
            Ty::Dict(k, x) => (true, strip_opt(x), Some((**k).clone())),
            _ => (false, t, None),
        };
        let keys = keys.map(|k| self.slot(&Slot::of(k), reached)).unwrap_or(J::Null);
        let unit01 = matches!(elem, Ty::Num(n) if n.is_rate() && n.step.is_some());
        let (record, fields): (bool, Vec<J>) = match elem {
            Ty::Record(id @ RecId::Module { .. }) => (true, self.p.fields(id).iter().map(|(n, fs)| json!([n.text, self.slot(fs, reached)])).collect()),
            _ => (false, Vec::new()),
        };
        // the fields of each value of an enum that has fields (the one value, for the type of a value)
        let sum = match elem {
            Ty::Enum(id) if self.p.is_sum(id) => Some((id.clone(), None)),
            Ty::Record(r @ RecId::Variant { value, .. }) => r.sum().map(|id| (id, Some(value.clone()))),
            _ => None,
        };
        // a result: `ok` holding `value`, `err` holding `error` (kaname's DESIGN 26.3)
        let result = match elem {
            Ty::Result(a, e) => Some(vec![json!(["ok", [["value", self.slot(&Slot::of((**a).clone()), reached)]]]), json!(["err", [["error", self.slot(&Slot::of((**e).clone()), reached)]]])]),
            _ => None,
        };
        let values: Vec<J> = match sum {
            _ if result.is_some() => result.unwrap_or_default(),
            Some((id, only)) => self
                .p
                .values(&id)
                .0
                .iter()
                .filter(|v| only.as_ref().is_none_or(|o| *o == v.text))
                .map(|v| json!([v.text, self.p.value_fields(&id, &v.text).iter().map(|(n, fs)| json!([n.text, self.slot(fs, reached)])).collect::<Vec<_>>()]))
                .collect(),
            None => Vec::new(),
        };
        let (count, ends) = if reached {
            (if list { filled(s.count.as_ref(), Some(whole_count())) } else { s.count.clone() }, filled(s.range.as_ref(), whole_range(elem)))
        } else {
            (s.count.clone(), s.range.clone())
        };
        json!({"list": list, "count": range(&count), "range": range(&ends), "unit01": unit01, "record": record, "fields": fields, "values": values, "keys": keys})
    }

    /// What a place declares, where it holds the values of a record or an enum that holds itself
    /// (kaname's DESIGN 28.1): its count, the keys of a dict, and the type's number in the table.
    fn nested_slot(&self, s: &Slot, reached: bool) -> Option<J> {
        let t = strip_opt(&s.ty);
        let (list, elem, keys) = match t {
            Ty::List(x) | Ty::Set(x) => (true, strip_opt(x), None),
            Ty::Dict(k, x) => (true, strip_opt(x), Some((**k).clone())),
            _ => (false, t, None),
        };
        let n = self.table(elem)?;
        let keys = keys.map(|k| self.slot(&Slot::of(k), reached)).unwrap_or(J::Null);
        let count = if reached && list { filled(s.count.as_ref(), Some(whole_count())) } else { s.count.clone() };
        Some(json!({"list": list, "count": range(&count), "range": null, "unit01": false, "record": false, "fields": [], "values": [], "keys": keys, "ref": n}))
    }

    /// How the interpreter compares two values of a type: by their places in an ordered enum, else as
    /// equal or not (numbers and dates it compares as such whatever this says).
    fn order(&self, t: &Ty) -> J {
        match t.present() {
            // a type parameter held to keys: what is compared is a number, a date, a string or a bool,
            // which the model compares by their values, or a value of an enum, by its place among the
            // values of the module's enums (`param_order`)
            Ty::Param(_) => json!({"enum": self.param_order()}),
            Ty::Num(_) => json!("num"),
            Ty::Date => json!("date"),
            // by their code points (kaname's DESIGN 25.1)
            Ty::Str => json!("str"),
            Ty::Enum(id) => json!({"enum": self.p.values(id).0.iter().map(|n| n.text.clone()).collect::<Vec<_>>()}),
            _ => json!("eq"),
        }
    }

    /// The order kaname puts the keys of a type in (kaname's DESIGN 24.1; `value.rs`'s `key_cmp`).
    fn key(&self, t: &Ty) -> J {
        match t.present() {
            Ty::Param(_) => json!({"enum": self.param_order()}),
            Ty::Num(_) => json!("num"),
            Ty::Date => json!("date"),
            Ty::Str => json!("str"),
            Ty::Bool => json!("bool"),
            Ty::Enum(id) => json!({"enum": self.p.values(id).0.iter().map(|n| n.text.clone()).collect::<Vec<_>>()}),
            Ty::Record(id) => json!({"record": self.p.fields(id).iter().map(|(_, s)| self.key(&s.ty)).collect::<Vec<_>>()}),
            _ => json!("num"),
        }
    }

    /// The values of every enum of the module and of the rules it reads, each enum's in its order: the
    /// order a value of a type parameter held to keys is in, where it is a value of an enum (the
    /// interpreter orders it by the type the call gives, kaname's DESIGN 26.4; the model has no types).
    /// Two enums with a value of one name in other orders would make it no order of either.
    fn param_order(&self) -> Vec<String> {
        let mut lists: Vec<Vec<String>> = self.c.decls.enums.iter().filter(|e| !e.is_sum()).map(|e| e.values.iter().map(|v| v.text.clone()).collect()).collect();
        for facts in self.p.rules.values() {
            for e in &facts.enums {
                lists.push(e.values.iter().map(|v| v.name.clone()).collect());
            }
        }
        let mut all: Vec<String> = Vec::new();
        for l in &lists {
            for v in l {
                if !all.contains(v) {
                    all.push(v.clone());
                }
            }
        }
        for l in &lists {
            let ranks: Vec<usize> = l.iter().map(|v| all.iter().position(|w| w == v).unwrap()).collect();
            assert!(ranks.windows(2).all(|w| w[0] < w[1]), "two enums of the module order the values {l:?} otherwise: no order a type parameter takes");
        }
        all
    }

    fn block(&self, b: &Block) -> J {
        let mut out = self.expr(&b.answer);
        for l in b.lets.iter().rev() {
            let from = self.ty(&l.value);
            let bound = self.c.typed.bound.get(&l.value.id).unwrap_or(from);
            out = json!({"let": [l.name, self.expr(&l.value), self.conv(from, bound, &l.value), out]});
        }
        out
    }

    fn bad(&self, e: &Expr, args: &[&Expr]) -> J {
        json!({"bad": [e.id, e.span.line, args.iter().map(|x| self.expr(x)).collect::<Vec<_>>()]})
    }

    fn expr(&self, e: &Expr) -> J {
        let (id, line) = (e.id, e.span.line);
        match &e.kind {
            ExprKind::Num(n) => {
                let v = if n.unit == "%" { n.value.div(Rat::int(100)) } else { n.value };
                json!({"num": [id, line, rat(v)]})
            }
            ExprKind::Date(d) => json!({"date": [id, d]}),
            ExprKind::Str(s) => json!({"str": [id, s]}),
            ExprKind::Bool(b) => json!({"bool": [id, b]}),
            ExprKind::None => json!({"none": id}),
            ExprKind::Var(w) => match self.rf(e) {
                Ref::EnumValue(eid, v) => self.enum_value(e, eid, v),
                // the value `ok x` or `err e` names: the field of the result the arm takes
                Ref::Local if self.payloads.borrow().iter().any(|(x, _, _)| x == w) => {
                    let (tmp, field) = self.payloads.borrow().iter().rev().find(|(x, _, _)| x == w).map(|(_, t, f)| (t.clone(), *f)).unwrap();
                    let at = self.fresh.get();
                    self.fresh.set(at + 1);
                    json!({"field": [id, line, {"var": [at, line, tmp]}, field]})
                }
                Ref::Local => json!({"var": [id, line, w]}),
                _ => self.bad(e, &[]),
            },
            ExprKind::Field(base, _, _) => match self.rf(e) {
                Ref::EnumValue(eid, v) => self.enum_value(e, eid, v),
                Ref::Field(name) => json!({"field": [id, line, self.expr(base), name]}),
                _ => self.bad(e, &[]),
            },
            ExprKind::Call(callee, args) => self.call(e, callee, args),
            ExprKind::Record(_, base, inits) => self.record(e, base.as_deref(), inits),
            ExprKind::List(xs) => {
                let et = match self.ty(e) {
                    Ty::List(t) => (**t).clone(),
                    _ => Ty::Error,
                };
                json!({"list": [id, xs.iter().map(|x| json!([self.expr(x), self.conv(self.ty(x), &et, x)])).collect::<Vec<_>>()]})
            }
            ExprKind::Dict(pairs) => {
                let (kt, vt) = match self.ty(e) {
                    Ty::Dict(k, v) => ((**k).clone(), (**v).clone()),
                    _ => (Ty::Error, Ty::Error),
                };
                let pairs: Vec<J> = pairs.iter().map(|(k, v)| json!([self.expr(k), self.conv(self.ty(k), &kt, k), self.expr(v), self.conv(self.ty(v), &vt, v)])).collect();
                json!({"dict": [id, self.key(&kt), pairs]})
            }
            ExprKind::Neg(x) => json!({"neg": [id, line, self.expr(x)]}),
            ExprKind::Not(x) => json!({"not": [id, line, self.expr(x)]}),
            ExprKind::In(x, _, _) => json!({"in": [id, line, self.expr(x), self.conv(self.ty(x), self.ty(e), e)]}),
            ExprKind::Bin(a, op, b) => self.binary(e, a, *op, b),
            ExprKind::If(c, t, f) => json!({"if": [
                id,
                self.expr(c),
                self.block(t),
                self.block(f),
                self.conv(self.ty(&t.answer), self.ty(e), &t.answer),
                self.conv(self.ty(&f.answer), self.ty(e), &f.answer),
            ]}),
            ExprKind::Match(s, arms) => {
                let st = self.ty(s);
                let result = matches!(st.present(), Ty::Result(..));
                let arms: Vec<J> = arms
                    .iter()
                    .map(|arm| {
                        let mark = self.payloads.borrow().len();
                        let pats: Vec<J> = arm
                            .patterns
                            .iter()
                            .map(|p| match p {
                                // `ok x` and `err e`: the result the arm takes, named apart, the
                                // value it holds read from it where the arm names it (kaname's
                                // DESIGN 26.3)
                                Pattern::Value(path, bind, _) if result => {
                                    let word = path[path.len() - 1].0.clone();
                                    let field = if word == "ok" { "value" } else { "error" };
                                    let tmp = bind.as_ref().map(|(b, _)| {
                                        let tmp = format!("{b}#{word}");
                                        self.payloads.borrow_mut().push((b.clone(), tmp.clone(), field));
                                        tmp
                                    });
                                    json!({"value": [word, tmp]})
                                }
                                Pattern::None(_) => json!("none"),
                                Pattern::Some(x, _) => json!({"some": x}),
                                Pattern::Bool(b, _) => json!({"bool": b}),
                                Pattern::Value(path, bind, _) => {
                                    let word = &path[path.len() - 1].0;
                                    let (name, sum) = match st.present() {
                                        Ty::Enum(id) => (self.p.values(id).0.iter().find(|n| n.is(word)).map(|n| n.text.clone()), self.p.is_sum(id)),
                                        _ => (None, false),
                                    };
                                    // a value of an enum that has fields, and the name the arm gives it
                                    if sum { json!({"value": [name, bind.as_ref().map(|(b, _)| b.clone())]}) } else { json!({"enum": name}) }
                                }
                                // a literal, as the value it is in the unit of what the `match` splits
                                Pattern::Lit(x) => json!({"lit": kaname::interp::literal(self.c, x, st).map(|v| val(&v)).unwrap_or(J::Null)}),
                                Pattern::Else(_) => json!("else"),
                            })
                            .collect();
                        let body = self.block(&arm.body);
                        self.payloads.borrow_mut().truncate(mark);
                        json!([pats, body, self.conv(self.ty(&arm.body.answer), self.ty(e), &arm.body.answer)])
                    })
                    .collect();
                json!({"match": [id, line, self.expr(s), arms]})
            }
            // a lambda made a value (kaname's DESIGN 27.1): the module's lambda (`lams`); a builtin's own
            // lambda is the builtin's, read where it is
            ExprKind::Lambda(..) if matches!(self.ty(e), Ty::Fn(..)) => json!({"lam": id}),
            ExprKind::Lambda(..) => self.bad(e, &[]),
            ExprKind::Block(b) => json!({"block": [id, self.block(b), self.conv(self.ty(&b.answer), self.ty(e), &b.answer)]}),
        }
    }

    fn binary(&self, e: &Expr, a: &Expr, op: BinOp, b: &Expr) -> J {
        let (id, line) = (e.id, e.span.line);
        match op {
            BinOp::And => return json!({"and": [id, line, self.expr(a), self.expr(b)]}),
            BinOp::Or => return json!({"or": [id, line, self.expr(a), self.expr(b)]}),
            _ => {}
        }
        let (ta, tb) = (self.ty(a), self.ty(b));
        if op.is_comparison() {
            let t = common(ta, tb);
            return json!({"cmp": [
                id,
                line,
                op.shown(),
                self.expr(a),
                self.expr(b),
                self.conv(ta, t, a),
                self.conv(tb, t, b),
                self.order(ta),
                step_of(ta).map(rat).unwrap_or(J::Null),
                step_of(tb).map(rat).unwrap_or(J::Null),
            ]});
        }
        if matches!((strip(ta), strip(tb)), (Ty::Date, Ty::Date)) {
            return json!({"ddiff": [id, self.expr(a), self.expr(b)]});
        }
        let te = self.ty(e);
        let (ca, cb) = match op {
            BinOp::Add | BinOp::Sub => (self.conv(ta, te, a), self.conv(tb, te, b)),
            BinOp::Div if has_unit(ta) && has_unit(tb) => {
                let t = common(ta, tb);
                (self.conv(ta, t, a), self.conv(tb, t, b))
            }
            _ => (J::Null, J::Null),
        };
        json!({"arith": [id, line, op.shown(), self.expr(a), self.expr(b), ca, cb, step_of(te).map(rat).unwrap_or(J::Null)]})
    }

    fn rule_index(&self, canon: &Path) -> usize {
        let mut rs = self.rules.borrow_mut();
        match rs.iter().position(|r| r == canon) {
            Some(i) => i,
            None => {
                rs.push(canon.to_path_buf());
                rs.len() - 1
            }
        }
    }

    fn date_index(&self, canon: &Path) -> usize {
        let mut ds = self.dates.borrow_mut();
        match ds.iter().position(|r| r == canon) {
            Some(i) => i,
            None => {
                ds.push(canon.to_path_buf());
                ds.len() - 1
            }
        }
    }

    /// The places of a rule's call, in the order its inputs travel: the inputs, then the lists it
    /// walks; each its name and type.
    fn rule_places(&self, canon: &Path) -> Vec<(String, Ty)> {
        let facts = &self.p.rules[canon];
        let mut places: Vec<(String, Ty)> = facts.inputs.iter().map(|c| (c.name.clone(), column_slot(c, canon).ty)).collect();
        for (seq, _, _) in &facts.elements {
            places.push((seq.clone(), Ty::List(Box::new(Ty::Record(RecId::Element(canon.to_path_buf(), seq.clone()))))));
        }
        places
    }

    fn call(&self, e: &Expr, callee: &Expr, args: &[Arg]) -> J {
        let (id, line) = (e.id, e.span.line);
        // a function value called (kaname's DESIGN 27.3): each argument read in its parameter's unit,
        // and every value of the type of its answer, marked as the type's (kaname's DESIGN 27.4)
        if let Ref::CallValue = self.rf(e) {
            let ps = match self.ty(callee) {
                Ty::Fn(ps, _, _) => ps.clone(),
                _ => Vec::new(),
            };
            let a: Vec<J> = args.iter().enumerate().map(|(i, a)| json!([self.expr(a.expr()), self.conv(self.ty(a.expr()), ps.get(i).unwrap_or(&Ty::Error), a.expr())])).collect();
            let top = abs(&self.an.slot(&Slot::of(self.ty(e).clone()), true));
            return json!({"callv": [id, line, self.expr(callee), a, top]});
        }
        match self.rf(callee) {
            Ref::Fn(canon, name) => {
                assert_eq!(canon, &self.c.canon, "a call of a function of another module");
                let g = self.c.decls.fns.iter().position(|f| f.name.text == *name).unwrap();
                let info = &self.c.decls.fns[g];
                // a generic function's parameters with the types the call gives (kaname's DESIGN 26.2)
                let names = kaname::names::tnames(&info.tparams);
                let targs = self.c.typed.inst.get(&e.id).cloned().unwrap_or_default();
                let a: Vec<J> = args
                    .iter()
                    .filter_map(|a| {
                        let (n, x) = named(a)?;
                        let k = info.params.iter().position(|p| p.name.is(n))?;
                        let pt = subst(&info.params[k].slot.ty, &names, &targs);
                        Some(json!([k, self.expr(x), self.conv(self.ty(x), &pt, x)]))
                    })
                    .collect();
                json!({"call": [id, line, g, a]})
            }
            Ref::Rule(canon) => {
                let r = self.rule_index(canon);
                self.rule_calls.borrow_mut().insert((line, source(self.c, callee.span)), r);
                let facts = &self.p.rules[canon];
                let places = self.rule_places(canon);
                let a: Vec<J> = args
                    .iter()
                    .filter_map(|a| {
                        let (n, x) = named(a)?;
                        let k = places
                            .iter()
                            .position(|(q, _)| q == n)
                            .or_else(|| facts.inputs.iter().position(|c| c.alias == n))
                            .or_else(|| facts.elements.iter().position(|(_, al, _)| al == n).map(|i| facts.inputs.len() + i))?;
                        Some(json!([k, self.expr(x), self.conv(self.ty(x), &places[k].1, x)]))
                    })
                    .collect();
                json!({"rule": [id, line, r, a]})
            }
            Ref::Date(canon, date) => {
                let f = self.date_index(canon);
                self.date_calls.borrow_mut().insert((line, source(self.c, callee.span)), (f, date.clone()));
                let facts = &self.p.dates[canon];
                let a: Vec<J> = args
                    .iter()
                    .filter_map(|a| {
                        let (n, x) = named(a)?;
                        let k = facts.inputs.iter().position(|i| i.name == n || i.alias == n)?;
                        let place = date_input_slot(facts, &facts.inputs[k].name)?.ty;
                        Some(json!([k, self.expr(x), self.conv(self.ty(x), &place, x)]))
                    })
                    .collect();
                json!({"datecall": [id, line, f, date, a]})
            }
            Ref::Transfer(t) => {
                let (_, params) = self.p.transfer(t).unwrap();
                let a: Vec<J> = args
                    .iter()
                    .filter_map(|a| {
                        let (n, x) = named(a)?;
                        let (pn, pt) = params.iter().find(|(p, _)| p == n)?;
                        Some(json!([pn, self.expr(x), self.conv(self.ty(x), pt, x), x.span.line, matches!(pt, Ty::Num(_))]))
                    })
                    .collect();
                json!({"transfer": [id, tid(t), params.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(), a]})
            }
            Ref::Builtin(name) => self.builtin(e, name, &args.iter().map(Arg::expr).collect::<Vec<_>>()),
            // a builtin written after a value: the value is its first argument (kaname's DESIGN 21.4)
            Ref::Method(name) => match &callee.kind {
                ExprKind::Field(base, _, _) => {
                    let mut xs: Vec<&Expr> = vec![&**base];
                    xs.extend(args.iter().map(Arg::expr));
                    self.builtin(e, name, &xs)
                }
                _ => self.bad(e, &args.iter().map(Arg::expr).collect::<Vec<_>>()),
            },
            _ => self.bad(e, &args.iter().map(Arg::expr).collect::<Vec<_>>()),
        }
    }

    /// A value of an enum written by its name: of an enum that has fields, a value with none, built
    /// as a record of no field with its name (kaname's DESIGN 23.6); else a value of the enum.
    fn enum_value(&self, e: &Expr, id: &EnumId, v: &str) -> J {
        if self.p.is_sum(id) { json!({"record": [e.id, e.span.line, true, [], [], v, null]}) } else { json!({"enum": [e.id, v]}) }
    }

    fn record(&self, e: &Expr, base: Option<&Expr>, inits: &[FieldInit]) -> J {
        let Ref::Record(id) = self.rf(e) else { return self.bad(e, &[]) };
        let fields = self.p.fields(id);
        let module = matches!(id, RecId::Module { .. } | RecId::Variant { .. });
        let tag = match id {
            RecId::Variant { value, .. } => json!(value),
            _ => J::Null,
        };
        let decl: Vec<J> = fields.iter().map(|(n, s)| json!([n.text, self.slot(s, false), matches!(s.ty, Ty::Num(_) | Ty::Date)])).collect();
        let given: Vec<J> = inits
            .iter()
            .filter_map(|fi| {
                let k = fields.iter().position(|(n, _)| n.is(&fi.name))?;
                Some(json!([k, self.expr(&fi.value), self.conv(self.ty(&fi.value), &fields[k].1.ty, &fi.value), fi.span.line]))
            })
            .collect();
        json!({"record": [e.id, e.span.line, module, decl, given, tag, base.map(|b| self.expr(b)).unwrap_or(J::Null)]})
    }

    fn builtin(&self, e: &Expr, name: &str, xs: &[&Expr]) -> J {
        let (id, line) = (e.id, e.span.line);
        let me = self.ty(e);
        let xs: Vec<&Expr> = xs.to_vec();
        let on_dict = matches!(self.ty(xs[0]).present(), Ty::Dict(..));
        match (name, xs.len()) {
            ("filter", 2) if on_dict => self.walk_k(e, "filter", &xs, None),
            ("last" | "sort_by" | "sort_by_desc" | "max_by" | "min_by" | "flat_map" | "filter_map" | "group_by" | "map_values", 2) => self.walk_k(e, name, &xs, None),
            ("to_dict", 3) => {
                let (Some((kp, kb)), Some((vp, vb))) = (lam(xs[1]), lam(xs[2])) else { return self.bad(e, &xs) };
                let (kt, vt) = dict_tys(me);
                json!({"to_dict": [id, line, self.key(&kt), self.expr(xs[0]), kp[0].0, self.expr(kb), self.conv(self.ty(xs[1]), &kt, xs[1]), vp[0].0, self.expr(vb), self.conv(self.ty(xs[2]), &vt, xs[2])]})
            }
            ("merge_with", 3) => {
                let Some((ps, body)) = lam(xs[2]) else { return self.bad(e, &xs) };
                let (kt, vt) = dict_tys(me);
                json!({"merge_with": [id, line, self.key(&kt), self.expr(xs[0]), self.expr(xs[1]), ps[0].0, ps[1].0, self.expr(body), self.conv(self.ty(xs[2]), &vt, xs[2])]})
            }
            ("count" | "sum" | "any" | "all" | "map" | "filter" | "first" | "max_of" | "min_of" | "running_totals", 2) => {
                let Some((ps, body)) = lam(xs[1]) else { return self.bad(e, &xs) };
                let w_ty = match me {
                    Ty::List(t) => match &**t {
                        Ty::Record(RecId::Running(_, w)) => (**w).clone(),
                        _ => Ty::Error,
                    },
                    _ => Ty::Error,
                };
                let to: Option<Ty> = match name {
                    "sum" => Some(me.clone()),
                    "max_of" | "min_of" => Some(me.present().clone()),
                    "map" => match me {
                        Ty::List(t) => Some((**t).clone()),
                        _ => None,
                    },
                    "running_totals" => match me {
                        Ty::List(t) => match &**t {
                            Ty::Record(RecId::Running(_, w)) => Some((**w).clone()),
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                };
                let c = to.map(|t| self.conv(self.ty(xs[1]), &t, xs[1])).unwrap_or(J::Null);
                let ord = match name {
                    "max_of" | "min_of" => self.order(me),
                    _ => json!("eq"),
                };
                let step = match name {
                    "sum" => step_of(me),
                    "running_totals" => step_of(&w_ty),
                    _ => None,
                };
                json!({"walk": [id, line, name, self.expr(xs[0]), ps[0].0, self.expr(body), c, ord, step.map(rat).unwrap_or(J::Null)]})
            }
            ("reduce", 3) => {
                let Some((ps, body)) = lam(xs[2]) else { return self.bad(e, &xs) };
                let ft = self.ty(xs[1]);
                json!({"reduce": [id, line, self.expr(xs[0]), self.expr(xs[1]), ps[0].0, ps[1].0, self.expr(body), self.conv(self.ty(xs[2]), ft, xs[2]), self.conv(ft, me, xs[1])]})
            }
            ("min" | "max", 2) => json!({"minmax": [id, line, name == "min", self.expr(xs[0]), self.expr(xs[1]), self.conv(self.ty(xs[0]), me, xs[0]), self.conv(self.ty(xs[1]), me, xs[1]), self.order(me)]}),
            ("mod", 2) => json!({"mod": [id, line, self.expr(xs[0]), self.expr(xs[1])]}),
            ("isqrt", 1) => json!({"isqrt": [id, line, self.expr(xs[0])]}),
            ("allocate", 3) => {
                let (tc, ts) = (self.ty(xs[1]), self.ty(xs[2]));
                let both = common(tc, ts);
                json!({"allocate": [id, line, self.expr(xs[0]), self.expr(xs[1]), self.expr(xs[2]), self.conv(tc, both, xs[1]), self.conv(ts, both, xs[2]), self.conv(self.ty(xs[0]), me, e)]})
            }
            ("up" | "down" | "half_up" | "half_down" | "half_even", 2) => {
                let tx = self.ty(xs[0]);
                json!({"round": [id, line, name, self.expr(xs[0]), self.expr(xs[1]), self.conv(self.ty(xs[1]), tx, xs[1]), self.conv(tx, me, e), abs(&self.an.top(me))]})
            }
            ("count", 1) => json!({"count1": [id, self.expr(xs[0])]}),
            // a result made: the value of the one field it holds (kaname's DESIGN 26.3)
            ("ok" | "err", 1) => {
                let Ty::Result(a, er) = me else { return self.bad(e, &xs) };
                let (field, inner) = if name == "ok" { ("value", (**a).clone()) } else { ("error", (**er).clone()) };
                let decl = json!([[field, self.slot(&Slot::of(inner.clone()), false), matches!(inner, Ty::Num(_) | Ty::Date)]]);
                let given = json!([[0, self.expr(xs[0]), self.conv(self.ty(xs[0]), &inner, xs[0]), line]]);
                json!({"record": [id, line, true, decl, given, name, null]})
            }
            // `char(n)`: the string of a constant code point (kaname's DESIGN 25.2)
            ("char", 1) => json!({"prim": [id, line, "char", "num", [[self.expr(xs[0]), null]], null]}),
            ("starts_with", 2) => json!({"starts_with": [id, self.expr(xs[0]), self.expr(xs[1])]}),
            // lists one after another, each read in the type of the answer
            ("concat", _) if matches!(me, Ty::List(_) | Ty::Empty) => {
                let args: Vec<J> = xs.iter().map(|x| json!([self.expr(x), self.conv(self.ty(x), me, x)])).collect();
                json!({"prim": [id, line, "concat", "num", args, null]})
            }
            ("concat", _) => json!({"concat": [id, xs.iter().map(|x| self.expr(x)).collect::<Vec<_>>()]}),
            ("numbers" | "join", 2) => {
                let args: Vec<J> = xs.iter().map(|x| json!([self.expr(x), null])).collect();
                json!({"prim": [id, line, name, "num", args, null]})
            }
            ("text", 1) => {
                let fmt = match self.ty(xs[0]) {
                    Ty::Num(nt) => match &nt.unit.dim {
                        Dim::Rate => json!("rate"),
                        Dim::Number | Dim::Count(_) => json!("number"),
                        _ => json!({"unit": nt.unit.unit}),
                    },
                    _ => json!("plain"),
                };
                json!({"text": [id, self.expr(xs[0]), fmt]})
            }
            _ => self.prim(e, name, &xs),
        }
    }

    /// A builtin of kaname's DESIGN 24.3 that applies a function to each element of a list, or to each
    /// entry or value of a dict: the function's answer read where the interpreter reads it, and the
    /// order of the keys it orders or holds.
    fn walk_k(&self, e: &Expr, name: &str, xs: &[&Expr], _: Option<()>) -> J {
        let (id, line) = (e.id, e.span.line);
        let me = self.ty(e);
        let Some((ps, body)) = lam(xs[1]) else { return self.bad(e, xs) };
        let lt = self.ty(xs[1]);
        let (c, key) = match name {
            "flat_map" => (self.conv(lt, me, xs[1]), json!("num")),
            "filter_map" => match me {
                Ty::List(t) => (self.conv(lt, &Ty::Opt(t.clone()), xs[1]), json!("num")),
                _ => (J::Null, json!("num")),
            },
            "group_by" => {
                let (kt, _) = dict_tys(me);
                (self.conv(lt, &kt, xs[1]), self.key(&kt))
            }
            "map_values" => {
                let (kt, vt) = dict_tys(me);
                (self.conv(lt, &vt, xs[1]), self.key(&kt))
            }
            "filter" => (J::Null, self.key(&dict_tys(me).0)),
            "sort_by" | "sort_by_desc" | "max_by" | "min_by" => (J::Null, self.key(lt)),
            _ => (J::Null, json!("num")),
        };
        json!({"walkk": [id, line, name, self.expr(xs[0]), ps[0].0, self.expr(body), c, key]})
    }

    /// A builtin of kaname's DESIGN 24.3 that takes no function: its arguments, the dict, the set or the
    /// list first, each read where `ranges.rs` and the interpreter read it; on what is no dict, set or
    /// list, every value of its answer's type.
    fn prim(&self, e: &Expr, name: &str, xs: &[&Expr]) -> J {
        let (id, line) = (e.id, e.span.line);
        let me = self.ty(e);
        let t0 = self.ty(xs[0]).clone();
        let raw = |x: &Expr| json!([self.expr(x), null]);
        let to = |x: &Expr, t: &Ty| json!([self.expr(x), self.conv(self.ty(x), t, x)]);
        let number = Ty::Num(NumTy::number());
        let (op, key, args): (&str, J, Vec<J>) = match &t0 {
            Ty::Dict(kt, vt) => {
                let op = match name {
                    "has" => "has_dict",
                    "remove" => "remove_dict",
                    x => x,
                };
                let args = match name {
                    "get" | "has" | "remove" => vec![raw(xs[0]), to(xs[1], kt)],
                    "put" => vec![raw(xs[0]), to(xs[1], kt), to(xs[2], vt)],
                    "merge" => vec![raw(xs[0]), to(xs[1], &t0)],
                    _ => vec![raw(xs[0])],
                };
                (op, self.key(kt), args)
            }
            Ty::Set(et) => {
                let op = match name {
                    "has" => "has_set",
                    "remove" => "remove_set",
                    x => x,
                };
                let args = match name {
                    "has" | "add" | "remove" => vec![raw(xs[0]), to(xs[1], et)],
                    "union" | "intersect" | "difference" => vec![raw(xs[0]), to(xs[1], &t0)],
                    _ => vec![raw(xs[0])],
                };
                (op, self.key(et), args)
            }
            Ty::List(et) => {
                let args = match name {
                    "at" => vec![raw(xs[0]), to(xs[1], &number)],
                    "append" | "contains" => vec![raw(xs[0]), to(xs[1], et)],
                    _ => xs.iter().map(|x| raw(x)).collect(),
                };
                (name, self.key(et), args)
            }
            // the builtins of a string (kaname's DESIGN 25.2): those a list has too by their own names
            Ty::Str => {
                let op = match name {
                    "slice" => "slice_text",
                    "take" => "take_text",
                    "drop" => "drop_text",
                    "contains" => "contains_text",
                    x => x,
                };
                (op, json!("str"), xs.iter().map(|x| raw(x)).collect())
            }
            _ => {
                // what is called on no dict, set or list the analysis follows: every value of its type
                let args: Vec<J> = xs.iter().map(|x| raw(x)).collect();
                return json!({"prim": [id, line, name, "num", args, abs(&self.an.top(me))]});
            }
        };
        json!({"prim": [id, line, op, key, args, null]})
    }

    /// The values of a numeric output of a rule over every input it takes (rulec's `output_values`),
    /// as `ranges::output_iv` reads them, and what any other output can be.
    fn output(&self, canon: &Path, c: &ritsu_ports::Column) -> Abs {
        let t = column_slot(c, canon).ty;
        let inner = match (&c.ty, &t) {
            (ritsu_ports::ColumnType::Opt(_), Ty::Opt(x)) => (**x).clone(),
            _ => t.clone(),
        };
        let v = match &inner {
            Ty::Num(_) => {
                let step = match strip(&t) {
                    Ty::Num(n) if n.is_rate() => n.step.unwrap_or(Rat::int(1)),
                    _ => Rat::int(1),
                };
                let den = step_of(&t).map(|s| if s.num == 0 { 1 } else { s.den });
                let iv = match self.suite.rules.as_ref().map(|r| r.output_values(&self.p.file(canon), &c.name)) {
                    Some(Ok(Found::Value(ov))) => {
                        let e = |v: Option<i128>| v.map(|i| Rat::int(i).mul(step));
                        Iv::new(e(ov.min), e(ov.max), den)
                    }
                    _ => Iv { den, ..Iv::TOP },
                };
                // an end the rule does not bound is the end of the type (kaname's DESIGN 32)
                let w = kaname::ty::whole_range(strip(&t));
                Abs::Num(Iv {
                    lo: iv.lo.or(w.as_ref().and_then(|w| w.min)),
                    hi: iv.hi.or(w.as_ref().and_then(|w| w.max)),
                    ..iv
                })
            }
            other => self.an.top(other),
        };
        if matches!(c.ty, ritsu_ports::ColumnType::Opt(_)) { Abs::Opt(true, Box::new(v)) } else { v }
    }

    fn rule_json(&self, canon: &Path) -> J {
        let facts = &self.p.rules[canon];
        let file = self.p.file(canon);
        let mut inputs: Vec<J> = facts
            .inputs
            .iter()
            .map(|c| {
                let slot = column_slot(c, canon);
                let numeric = matches!(strip(&slot.ty), Ty::Num(_));
                let r = if numeric { range(&slot.range) } else { J::Null };
                let step = match strip(&slot.ty) {
                    Ty::Num(n) if n.is_rate() => n.step.map(rat).unwrap_or(J::Null),
                    Ty::Num(_) => rat(Rat::int(1)),
                    _ => J::Null,
                };
                // X3 (a) holds a day given to a date input to the range it declares; an input whose
                // range is a koyomi date takes the date's days as a precondition, which X2 decides
                let days = facts.preconditions.iter().any(|p| matches!(p, ritsu_ports::Precondition::Days { input, .. } if *input == c.name));
                let date_ranged = matches!(strip(&slot.ty), Ty::Date)
                    && !days
                    && self.suite.rules.as_ref().and_then(|r| r.date_range(&file, &c.name).ok()).is_some_and(|(lo, hi)| lo.is_some() || hi.is_some());
                // an input of an ordered enum held to a range: the values it lets in (ritsu's DESIGN 3.2)
                let values = if matches!(strip(&slot.ty), Ty::Enum(_)) { column_values(c).map(|vs| json!(vs)).unwrap_or(J::Null) } else { J::Null };
                json!({"name": c.name, "range": r, "step": step, "opt": matches!(slot.ty, Ty::Opt(_)), "date_ranged": date_ranged, "values": values, "elements": null})
            })
            .collect();
        for (seq, _, cols) in &facts.elements {
            let fields: Vec<J> = cols
                .iter()
                .map(|c| {
                    let slot = column_slot(c, canon);
                    json!([c.name, if matches!(strip(&slot.ty), Ty::Num(_)) { range(&slot.range) } else { J::Null }])
                })
                .collect();
            inputs.push(json!({"name": seq, "range": null, "step": null, "opt": false, "date_ranged": false, "values": null, "elements": fields}));
        }
        json!({
            "name": facts.rule,
            "inputs": inputs,
            "outputs": facts.outputs.iter().map(|c| json!([c.name, abs(&self.output(canon, c))])).collect::<Vec<_>>(),
            "pres": facts.preconditions.iter().map(kaname::ranges::said).collect::<Vec<_>>(),
        })
    }

    fn dates_json(&self, canon: &Path) -> J {
        let facts = &self.p.dates[canon];
        let file = self.p.file(canon);
        let dates = self.suite.dates.as_ref().unwrap();
        json!({
            "inputs": facts.inputs.iter().map(|i| json!({"name": i.name, "date": i.kind == ritsu_ports::DateKind::Date, "min": i.min, "max": i.max})).collect::<Vec<_>>(),
            "dates": facts.functions.iter().map(|d| {
                let hull = match dates.values(&file, &d.name) {
                    Ok(Found::Value(set)) if !set.is_empty() => json!([set.iter().next().unwrap(), set.iter().next_back().unwrap()]),
                    _ => J::Null,
                };
                let days = match dates.days(&file, &d.name) {
                    Ok(Found::Value((fewest, most))) => json!([fewest, most]),
                    _ => J::Null,
                };
                json!({"name": d.name, "reads": d.params, "hull": hull, "days": days})
            }).collect::<Vec<_>>(),
        })
    }

    fn function(&self, f: usize) -> J {
        let info = &self.c.decls.fns[f];
        let decl = &self.c.module.fns[f];
        json!({
            "name": info.name.text,
            "line": info.line,
            "params": info.params.iter().map(|prm| json!({
                "name": prm.name.text,
                "slot": self.slot(&prm.slot, true),
                "scalar": matches!(prm.slot.ty, Ty::Num(_) | Ty::Date),
                "abs": abs(&self.an.slot(&prm.slot, true)),
            })).collect::<Vec<_>>(),
            "ret": self.slot(&info.ret, false),
            "ranged": info.ret.range.is_some() || info.ret.count.is_some(),
            "declared": abs(&self.an.slot(&info.ret, false)),
            "body": self.block(&decl.body),
            "answer": decl.body.answer.id,
            "answer_line": decl.body.answer.span.line,
            "conv": self.conv(self.ty(&decl.body.answer), &info.ret.ty, &decl.body.answer),
            "reread": info.reread(),
            "takes_fn": info.takes_fn(),
        })
    }

    /// The lambdas the module makes values of (kaname's DESIGN 27.1): each its id, the function it is
    /// written in, its parameters with every value of their types (marked as the types'), its body,
    /// and its body's answer read in the type of its answer.
    fn lams(&self) -> Vec<J> {
        let mut out = Vec::new();
        for (f, decl) in self.c.module.fns.iter().enumerate() {
            let mut es = Vec::new();
            decl.body.walk(&mut es);
            for e in es {
                let ExprKind::Lambda(ps, body) = &e.kind else { continue };
                let Ty::Fn(pts, r, _) = self.ty(e) else { continue };
                let params: Vec<J> = ps.iter().zip(pts).map(|((p, _), t)| json!([p, abs(&self.an.slot(&Slot::of(t.clone()), true))])).collect();
                out.push(json!({"id": e.id, "fn": f, "params": params, "body": self.expr(body), "conv": self.conv(self.ty(body), r, body)}));
            }
        }
        out
    }

    fn module(&self) -> J {
        let fns: Vec<J> = (0..self.c.decls.fns.len()).map(|f| self.function(f)).collect();
        let lams = self.lams();
        let rules: Vec<J> = self.rules.borrow().clone().iter().map(|r| self.rule_json(r)).collect();
        let dates: Vec<J> = self.dates.borrow().clone().iter().map(|d| self.dates_json(d)).collect();
        let rings: Vec<J> = kaname::effects::rings(self.c).into_iter().map(|(r, round)| json!([r, round])).collect();
        // the table of the types that hold themselves, last: every place above has asked for its own
        let slots: Vec<J> = self.slots.borrow().iter().map(|(_, j)| j.clone()).collect();
        json!({"fns": fns, "rules": rules, "dates": dates, "rings": rings, "lams": lams, "slots": slots})
    }

    // ── A run's calls, as the oracle of the model ───────────────────────────

    /// A value off rulec's or koyomi's wire, as the model reads it: a number in the unit of its type.
    fn wire(&self, j: &RJson, t: &Ty) -> J {
        match t {
            Ty::Opt(x) => {
                if j.is_null() {
                    J::Null
                } else {
                    self.wire(j, x)
                }
            }
            Ty::Num(n) => {
                let i = j.as_int().expect("a whole number on the wire");
                let r = if n.is_rate() { Rat::int(i).mul(n.step.unwrap()) } else { Rat::int(i) };
                json!({"n": rat(r)})
            }
            Ty::Bool => json!({"b": j.as_bool().unwrap()}),
            Ty::Str => json!({"s": j.as_str().unwrap()}),
            Ty::Date => json!({"d": kaname::date::parse(j.as_str().unwrap()).unwrap()}),
            Ty::Enum(_) => json!({"e": j.as_str().unwrap()}),
            Ty::List(x) => {
                let Ty::Record(id) = &**x else { panic!("a list of no records on the wire") };
                let fields = self.p.fields(id);
                json!({"l": j.as_arr().unwrap().iter().map(|el| json!({"r": fields.iter().map(|(f, s)| json!([f.text, self.wire(el.get(&f.text).unwrap_or(&RJson::Null), &s.ty)])).collect::<Vec<_>>()})).collect::<Vec<_>>()})
            }
            _ => J::Null,
        }
    }

    /// What the rules and the dates answered on a run, from the interpreter's trace.
    fn oracle(&self, it: &Interp) -> (Vec<J>, Vec<J>) {
        let (mut rules, mut dates) = (Vec::new(), Vec::new());
        for call in it.trace.calls() {
            let j = &call.json;
            let line = j.get("line").and_then(RJson::as_int).unwrap() as usize;
            if let Some(name) = j.get("rule").and_then(RJson::as_str) {
                let r = self.rule_calls.borrow()[&(line, name.to_string())];
                let canon = self.rules.borrow()[r].clone();
                let input = j.get("in").unwrap();
                let ins: Vec<J> = self.rule_places(&canon).iter().map(|(n, t)| self.wire(input.get(n).unwrap_or(&RJson::Null), t)).collect();
                let outs = match j.get("out") {
                    Some(out) => {
                        let facts = &self.p.rules[&canon];
                        json!(facts.outputs.iter().map(|c| self.wire(out.get(&c.name).unwrap_or(&RJson::Null), &column_slot(c, &canon).ty)).collect::<Vec<_>>())
                    }
                    None => J::Null,
                };
                rules.push(json!([r, ins, outs]));
            } else if let Some(name) = j.get("date").and_then(RJson::as_str) {
                let (f, date) = self.date_calls.borrow()[&(line, name.to_string())].clone();
                let given: Vec<J> = j
                    .get("in")
                    .and_then(RJson::as_obj)
                    .unwrap()
                    .iter()
                    .map(|(n, v)| json!([n, v.as_str().map(|s| kaname::date::parse(s).unwrap() as i128).or_else(|| v.as_int()).unwrap() as i64]))
                    .collect();
                let day = j.get("out").and_then(RJson::as_str).map(|s| json!(kaname::date::parse(s).unwrap())).unwrap_or(J::Null);
                dates.push(json!([f, date, given, day]));
            }
        }
        (rules, dates)
    }
}

/// An argument written with its name: the name and the expression.
fn named(a: &Arg) -> Option<(&str, &Expr)> {
    match a {
        Arg::Named(n, _, x) => Some((n.as_str(), x)),
        Arg::Pos(_) => None,
    }
}

/// The types of a dict's keys and of its values.
fn dict_tys(t: &Ty) -> (Ty, Ty) {
    match t.present() {
        Ty::Dict(k, v) => ((**k).clone(), (**v).clone()),
        _ => (Ty::Error, Ty::Error),
    }
}

/// A function given to a builtin: its parameters and its body.
fn lam(x: &Expr) -> Option<(&[(String, Span)], &Expr)> {
    match &x.kind {
        ExprKind::Lambda(ps, body) => Some((ps.as_slice(), &**body)),
        _ => None,
    }
}

/// How deep JSON nests its arrays and objects.
fn depth_of(j: &J) -> usize {
    match j {
        J::Array(xs) => 1 + xs.iter().map(depth_of).max().unwrap_or(0),
        J::Object(fs) => 1 + fs.values().map(depth_of).max().unwrap_or(0),
        _ => 0,
    }
}

/// What the interpreter answers for a run, in the model's words.
fn answer(out: Result<Value, Stop>) -> J {
    match out {
        Ok(v) => json!({"value": val(&v)}),
        Err(Stop::Failed(f)) => json!({"fails": [f.kind.word(), f.line]}),
        Err(Stop::Diverged(_)) => json!({"diverged": true}),
    }
}

/// The relations each call of a rule keeps between the inputs it gives, by the call's expression (what
/// `Programs` hands ritsu-cross), in the order of the expressions.
fn kept_json(ff: &kaname::ranges::FnFacts) -> Vec<J> {
    let mut out: Vec<(usize, J)> = ff
        .sites
        .iter()
        .filter(|s| matches!(s.kind, kaname::ranges::SiteKind::Rule { .. }))
        .map(|s| (s.expr, json!([s.expr, s.relations.iter().map(|(a, r, b)| json!([a, r.shown(), b])).collect::<Vec<_>>()])))
        .collect();
    out.sort_by_key(|(e, _)| *e);
    out.into_iter().map(|(_, j)| j).collect()
}

/// What the analysis of a module comes to, in the model's words: per function its result, its
/// places by expression and key, its effect, the relations its calls of rules keep; and the values of
/// every expression, by id.
fn analysis_json(c: &Checked) -> (Vec<J>, Vec<J>) {
    let a = c.analysis.get().unwrap();
    let fns = a
        .fns
        .iter()
        .map(|ff| {
            // the places of the function, and those the readings again of its calls found (kaname's
            // DESIGN 26.5), which the model keeps with the function's own
            let mut places: Vec<J> = ff
                .places
                .iter()
                .chain(ff.rplaces.iter().filter(|r| r.module == c.canon).map(|r| &r.place))
                .map(|pl| json!([pl.expr, key_of(&pl.what), pl.kind.word(), abs(&pl.value), pl.outcome == Outcome::Shown, pl.outcome == Outcome::Pending]))
                .collect();
            places.sort_by_key(|p| (p[0].as_u64(), p[1].as_str().map(str::to_string)));
            json!({
                "result": abs(&ff.result),
                "places": places,
                "effect": [
                    ff.effect.fails.iter().map(|k| k.word()).collect::<Vec<_>>(),
                    ff.effect.decided.iter().map(|k| k.word()).collect::<Vec<_>>(),
                    ff.effect.diverges,
                ],
                "kept": kept_json(ff),
            })
        })
        .collect();
    let vals = a.at.iter().enumerate().filter_map(|(i, x)| x.as_ref().map(|x| json!([i, abs(x)]))).collect();
    (fns, vals)
}

/// The verdicts of the checks across the borders as kaname took them (`ranges.rs`'s `across`): each
/// place they decide that has one, by its expression and key, and whether it holds there.
fn verdicts_json(c: &Checked) -> (J, usize) {
    let a = c.analysis.get().unwrap();
    let mut out = Vec::new();
    for ff in &a.fns {
        for pl in &ff.places {
            if pl.what.across().is_some() && pl.outcome != Outcome::Pending {
                out.push(json!([pl.expr, key_of(&pl.what), pl.outcome == Outcome::Shown]));
            }
        }
    }
    let n = out.len();
    (J::Array(out), n)
}

/// One module ready to compare: its form for the model, the analysis kaname finds, and every run.
struct Prepared {
    name: String,
    module: String,
    /// The first id of an expression the model is given that the module does not have.
    made: usize,
    fns: Vec<J>,
    names: Vec<String>,
    vals: Vec<J>,
    runs: Vec<(String, String)>,
    seen: usize,
    outside: Vec<String>,
    verdicts: usize,
}

fn prepare(name: &str, c: &Rc<Checked>, suite: &Suite) -> Prepared {
    let p = Program::of(c);
    let ex = Ex::new(c, &p, suite);
    let mut module = ex.module();
    let (verdicts, nv) = verdicts_json(c);
    module["verdicts"] = verdicts;
    let (fns, vals) = analysis_json(c);
    let names: Vec<String> = c.decls.fns.iter().map(|f| f.name.text.clone()).collect();
    let a = c.analysis.get().unwrap();
    let (mut runs, mut seen, mut outside) = (Vec::new(), 0usize, Vec::new());
    for (f, fname) in names.iter().enumerate() {
        let (mut vs, _) = kaname::vectors::choose(&p, c, f);
        // a value of a type that holds itself nested as deep as a host may give one, and one deeper,
        // which the entry fails (kaname's DESIGN 28.1): the model's entry follows the table
        if !c.decls.fns[f].inner() {
            for (i, prm) in c.decls.fns[f].params.iter().enumerate() {
                for n in [kaname::interp::NESTED, kaname::interp::NESTED + 1] {
                    if let Some(v) = kaname::vectors::nested_value(&p, &prm.slot.ty, n) {
                        let mut input = kaname::vectors::base_input(&p, c, f);
                        input[i] = v;
                        vs.push(kaname::vectors::Vector { input, why: None });
                    }
                }
            }
        }
        for v in &vs {
            let mut it = Interp::new(&p, suite);
            let out = match it.check_entry(c, f, &v.input) {
                Err(fl) => Err(Stop::Failed(fl)),
                Ok(()) => {
                    it.watch = Some(Watch::default());
                    it.call(c, f, v.input.clone(), 0)
                }
            };
            if let Some(w) = it.watch.take() {
                for (file, id, value) in w.seen {
                    assert_eq!(file, c.canon, "a value of another module");
                    seen += 1;
                    match a.at.get(id).cloned().flatten() {
                        Some(x) if x.has(&value) => {}
                        other => outside.push(format!("{name}, {fname}: expression {id} is {value:?}, and the analysis says {other:?}")),
                    }
                }
            }
            let (rules, dates) = ex.oracle(&it);
            let line = json!({"run": f, "input": v.input.iter().map(val).collect::<Vec<_>>(), "rules": rules, "dates": dates});
            let said = answer(out);
            // an answer nested deeper than the comparison reads JSON is not compared: a value of a type
            // that holds itself, answered as deep as it came in
            if depth_of(&said) > 100 {
                continue;
            }
            runs.push((line.to_string(), said.to_string()));
        }
    }
    Prepared { name: name.to_string(), module: module.to_string(), made: c.module.exprs, fns, names, vals, runs, seen, outside, verdicts: nv }
}

/// Run `f` on a stack as deep as the recursions of the test modules need.
fn deep<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap()
}

/// The model's analysis of a module, compared with kaname's: what differs, each said once.
fn differences(pr: &Prepared, model: &J) -> Vec<String> {
    let mut out = Vec::new();
    let Some(mfns) = model.get("fns").and_then(J::as_array) else {
        return vec![format!("{}: the model answers no analysis: {model}", pr.name)];
    };
    if mfns.len() != pr.fns.len() {
        out.push(format!("{}: {} functions, and the model analyses {}", pr.name, pr.fns.len(), mfns.len()));
    }
    for (f, (ours, theirs)) in pr.fns.iter().zip(mfns).enumerate() {
        let fname = &pr.names[f];
        if ours["result"] != theirs["result"] {
            out.push(format!("{}, {fname}: the result is {} in Rust and {} in Lean", pr.name, ours["result"], theirs["result"]));
        }
        if ours["effect"] != theirs["effect"] {
            out.push(format!("{}, {fname}: the effect is {} in Rust and {} in Lean", pr.name, ours["effect"], theirs["effect"]));
        }
        let mut mk: Vec<J> = theirs["kept"].as_array().cloned().unwrap_or_default();
        mk.sort_by_key(|k| k[0].as_u64());
        if ours["kept"] != J::Array(mk.clone()) {
            out.push(format!("{}, {fname}: the calls of rules keep {} in Rust and {} in Lean", pr.name, ours["kept"], J::Array(mk)));
        }
        let mut mp: Vec<J> = theirs["places"].as_array().cloned().unwrap_or_default();
        mp.sort_by_key(|p| (p[0].as_u64(), p[1].as_str().map(str::to_string)));
        let rp = ours["places"].as_array().cloned().unwrap_or_default();
        let key = |p: &J| format!("{} {}", p[0], p[1]);
        for p in &rp {
            match mp.iter().find(|q| key(q) == key(p)) {
                Some(q) if q == p => {}
                Some(q) => out.push(format!("{}, {fname}: the place {} is {p} in Rust and {q} in Lean", pr.name, key(p))),
                None => out.push(format!("{}, {fname}: the place {} ({p}) is not in Lean", pr.name, key(p))),
            }
        }
        for q in &mp {
            if !rp.iter().any(|p| key(p) == key(q)) {
                out.push(format!("{}, {fname}: the place {} ({q}) is not in Rust", pr.name, key(q)));
            }
        }
    }
    let mut mv: Vec<J> = model.get("vals").and_then(J::as_array).cloned().unwrap_or_default();
    // an expression the model is given that the module does not have: the name of a result an arm
    // takes (`payloads`), whose value the field read from it is
    mv.retain(|v| v[0].as_u64().is_some_and(|i| (i as usize) < pr.made));
    mv.sort_by_key(|v| v[0].as_u64());
    for v in &pr.vals {
        match mv.iter().find(|w| w[0] == v[0]) {
            Some(w) if w == v => {}
            Some(w) => out.push(format!("{}: expression {} is {} in Rust and {} in Lean", pr.name, v[0], v[1], w[1])),
            None => out.push(format!("{}: expression {} ({}) has no value in Lean", pr.name, v[0], v[1])),
        }
    }
    for w in &mv {
        if !pr.vals.iter().any(|v| v[0] == w[0]) {
            out.push(format!("{}: expression {} ({}) has no value in Rust", pr.name, w[0], w[1]));
        }
    }
    out
}

#[test]
fn every_run_and_the_analysis_of_every_module_come_out_as_the_model_says() {
    let bin = program();
    if !ready(Need::Lean, || bin.exists(), "proofs/ is not built (lake build makes ritsu-model)") {
        return;
    }
    let prepared = deep(|| {
        let suite = suite();
        let dir = kaname_dir();
        let mut out = Vec::new();
        for f in EXAMPLES {
            let path = dir.join(f);
            let o = Run::new(suite.clone()).file(&path.to_string_lossy()).unwrap();
            let c = o.analysed.clone().unwrap_or_else(|| panic!("{f} does not check"));
            out.push(prepare(f, &c, &suite));
        }
        for m in MODULES {
            let src = std::fs::read_to_string(dir.join(format!("tests/modules/{m}.kn"))).unwrap();
            let o = Run::new(suite.clone()).text(&dir.join("t.kn").to_string_lossy(), &src);
            let c = o.analysed.clone().unwrap_or_else(|| panic!("{m} does not check: {:?}", o.diags));
            out.push(prepare(m, &c, &suite));
        }
        // the modules made from the examples, read while their directory is there
        let made = TempDir::new("model-kaname-materials");
        for (name, kn) in materials(&made) {
            let o = Run::new(suite.clone()).file(&kn.to_string_lossy()).unwrap();
            let c = o.analysed.clone().unwrap_or_else(|| panic!("{name} does not check: {:?}", o.diags));
            out.push(prepare(&name, &c, &suite));
        }
        out
    });
    let tmp = TempDir::new("model-kaname");
    let (mut lines, mut places, mut exprs, mut seen, mut verdicts) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut wrong = Vec::new();
    for (i, pr) in prepared.iter().enumerate() {
        let file = tmp.write(&format!("{i}.json"), pr.module.clone());
        // the analysis: one line, its answer kept to compare piece by piece
        let got: Mutex<Option<String>> = Mutex::new(None);
        let keep = |_: &str, b: &str| {
            *got.lock().unwrap() = Some(b.to_string());
            true
        };
        compare(&bin, "kaname", &file, std::iter::once((json!({"analysis": true}).to_string(), String::new())), &keep).unwrap_or_else(|e| panic!("{}: {e}", pr.name));
        let model: J = serde_json::from_str(got.lock().unwrap().as_deref().unwrap_or("null")).unwrap_or(J::Null);
        let diffs = differences(pr, &model);
        // the runs
        let c = compare(&bin, "kaname", &file, pr.runs.clone().into_iter(), &|a, b| serde_json::from_str::<J>(a).ok() == serde_json::from_str::<J>(b).ok()).unwrap_or_else(|e| panic!("{}: {e}", pr.name));
        let np: usize = pr.fns.iter().map(|f| f["places"].as_array().map_or(0, Vec::len)).sum();
        println!(
            "compared kaname {}: {} functions, {np} places ({} with a verdict), {} expressions, {} runs ({} differ), {} values seen",
            pr.name,
            pr.fns.len(),
            pr.verdicts,
            pr.vals.len(),
            c.lines,
            c.differ,
            pr.seen
        );
        if !diffs.is_empty() {
            wrong.push(format!("{}: the analysis differs from the model in {} ways; the first:\n  {}", pr.name, diffs.len(), diffs.iter().take(8).cloned().collect::<Vec<_>>().join("\n  ")));
        }
        if c.differ > 0 {
            wrong.push(c.report(&format!("{} runs", pr.name)));
        }
        if !pr.outside.is_empty() {
            wrong.push(format!("{}: {} values outside what the analysis says; the first:\n  {}", pr.name, pr.outside.len(), pr.outside.iter().take(5).cloned().collect::<Vec<_>>().join("\n  ")));
        }
        lines += c.lines;
        places += np;
        exprs += pr.vals.len();
        seen += pr.seen;
        verdicts += pr.verdicts;
    }
    println!(
        "compared kaname: {} modules, {lines} runs, {places} places ({verdicts} with a verdict), {exprs} expressions, {seen} values seen",
        prepared.len()
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(prepared.len(), EXAMPLES.len() + MODULES.len() + 10, "the modules are fewer than they were");
    assert!(lines >= 600 && places >= 180 && exprs >= 800 && verdicts >= 25, "fewer than they were: {lines} runs, {places} places, {exprs} expressions, {verdicts} verdicts");
}
