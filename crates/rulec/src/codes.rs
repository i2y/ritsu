//! The ledger of every diagnostic code (§11).
//!
//! §11 principle 5 makes the code and the JSON shape a stable API while the prose may
//! improve. That only holds if there is **one** place that knows the ledger, so this file
//! is it: `rulec explain` renders from here, `docs/codes.md` and `docs/codes.ja.md` are
//! that rendering checked in, and the tests refuse a code that the checker emits but the
//! ledger does not carry.
//!
//! Every entry carries a runnable `example`. A test feeds each one to `check_source` and
//! requires the code to come out, so an example cannot rot into a lie while the prose
//! around it still reads well.

use crate::diag::Severity;
use crate::json;

pub struct Entry {
    pub code: &'static str,
    pub severity: Severity,
    /// One line in business words — the same sentence shape the diagnostic itself uses,
    /// with the row numbers and names left out.
    pub title: String,
    /// What has to be true for this to be printed.
    pub when: String,
    /// How to get rid of it, down to the rewritten form (§11 principle 3).
    pub fix: String,
    /// The smallest `.rule` that produces it.
    pub example: &'static str,
    /// Only E109 needs this: with the default budget the example would have to be enormous,
    /// so it is reproduced with a budget this small.
    pub budget: Option<i64>,
    pub related: &'static [&'static str],
}

fn e(
    code: &'static str,
    severity: Severity,
    title: String,
    when: String,
    fix: String,
    example: &'static str,
    related: &'static [&'static str],
) -> Entry {
    Entry { code, severity, title, when, fix, example, budget: None, related }
}

fn err(
    code: &'static str,
    title: String,
    when: String,
    fix: String,
    example: &'static str,
    related: &'static [&'static str],
) -> Entry {
    e(code, Severity::Error, title, when, fix, example, related)
}

impl Entry {
    /// Only E109 uses this (see the field).
    fn with_budget(mut self, b: i64) -> Entry {
        self.budget = Some(b);
        self
    }
}

fn warn(
    code: &'static str,
    title: String,
    when: String,
    fix: String,
    example: &'static str,
    related: &'static [&'static str],
) -> Entry {
    e(code, Severity::Warning, title, when, fix, example, related)
}

// ── The examples ─────────────────────────────────────────────────────────
//
// Kept as constants so the ledger below reads as prose. Each one is the smallest file
// that reaches the code, and a test runs every one of them.

const X_E001: &str = "rule t(t) v1\ndescription \"unterminated\n";
const X_E002: &str = "rule t(t) v1\n\ninputs\n  @x(x) : bool\n";
const X_E003: &str = "inputs\n  x(x) : bool\n";
const X_E004: &str = "rule t(t) v1\n\n= 1\n";
const X_E005: &str = "rule t(t) v1\n\nfoo bar\n";
const X_E006: &str = "rule t(t) v1\n\nenum k(k) a(a) | b(b)\n";
const X_E007: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy any\n| x | -> r(r) : bool |\n| - | true |\n";
const X_E008: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : bool |\n|   | true |\n";
const X_E009: &str = "rule t(t) v1\n\nenum range(kind) = a(a) | b(b)\n";
const X_E010: &str = "rule t(t) v1\n\ninputs\n  w(w) : mass[g]  range >=0g <=10kg\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| w | -> r(r) : bool |\n\
                      | 0g..1000g | true |\n| >1000g | false |\n";
const X_E011: &str = "rule t(t) v1\n\ninputs\n  重量 : bool\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| 重量 | -> r(r) : bool |\n| - | true |\n";
const X_E012: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| y | -> r(r) : bool |\n| - | true |\n";
const X_E013: &str = "rule t(t) v1\n\nimport std/nope\n";
const X_E014: &str = "rule t(t) v1\n\ninputs\n  p(p) : money[円, incl_tax]  range >=0円 <=1万円\n  \
                      r(r) : rate[step 1%]  range >=0% <=100%\n\n\
                      outputs\n  o(o) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy first\n| r | -> o(o) : money[円, incl_tax] |\n\
                      | <=5% | 0円 |\n| - | p × r |\n";

const X_E015: &str = "rule t(t) v1\n\ninputs\n  p(p) : money[円, incl_tax]  range >=0円 <=1万円\n\n\
                      outputs\n  a(a) : money[円, incl_tax]  round down(1円)\n  \
                      b(b) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| p | -> a(a) : money[円, incl_tax] |\n| - | 100円 |\n\n\
                      result b = p\n";
const X_E016: &str = "rule t(t) v1\n\ninputs\n  p(p) : money[円, incl_tax]  range >=0円 <=1万円\n\n\
                      outputs\n  a(a) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| p | -> a(a) : money[円, incl_tax] |\n| - | 100円 |\n\n\
                      result a = p\nresult a = p + 100円\n";

const X_E017: &str = "rule t(t) v1\n\ninputs\n  a(a) : number  range >=0 <=10\n  \
                      b(b) : number  range >=0 <=10\n\nconstraint a\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| a | -> r(r) : bool |\n| - | true |\n";
const X_E018: &str = "rule t(t) v1\n\ninputs\n  a(a) : number  range >=0 <=10\n\n\
                      constraint a <= r\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| a | -> r(r) : bool |\n| - | true |\n";
const X_E019: &str = "rule t(t) v1\n\ninputs\n  a(a) : number  range >=0 <=10\n  \
                      b(b) : number  range >=0 <=10\n\nconstraint a <= b\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| a | b | -> r(r) : bool |\n| - | - | true |\n\n\
                      examples\n| a | b | -> r |\n| 5 | 1 | true |\n";

/// A rule that walks a sequence: an element decides a verdict, and the fold reduces the
/// column of verdicts. The three examples below bend one thing each.
#[allow(dead_code)]
const FOLD_HEAD: &str = "rule t(t) v1\n\n\
                      enum v(v) = a(a) | b(b)\n\n\
                      elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5円 | a |\n| >5円 | b |\n\n\
                      fold d over xs\n";
const X_E022: &str = const_str_e022();
const X_E023: &str = const_str_e023();
const X_E024: &str = const_str_e024();
const X_E025: &str = const_str_e025();
const fn const_str_e022() -> &'static str {
    // Every arm and `exhausted`, but no `empty`.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5円 | a |\n| >5円 | b |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  exhausted -> held\n"
}

const fn const_str_e023() -> &'static str {
    // Every arm and `empty`, but no `exhausted`.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5円 | a |\n| >5円 | b |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n"
}

const fn const_str_e024() -> &'static str {
    // Both answers, but the verdict `b` has no arm.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5円 | a |\n| >5円 | b |\n\n\
     fold d over xs\n  a -> next\n  empty -> 0円\n  exhausted -> held\n"
}

const fn const_str_e025() -> &'static str {
    // Examples for a rule that walks a sequence, with no column saying which one.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5円 | a |\n| >5円 | b |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n  exhausted -> held\n\n\
     examples\n| -> r |\n| 0円 |\n"
}

const fn const_str_e026() -> &'static str {
    // A sequence whose column is not a field of an element.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5円 | a |\n| >5円 | b |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n  exhausted -> held\n\n\
     sequence s(s)\n| m |\n| 3円 |\n"
}

const fn const_str_e027() -> &'static str {
    // An example naming a sequence nobody wrote.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5円 | a |\n| >5円 | b |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n  exhausted -> held\n\n\
     examples\n| xs | -> r |\n| nope | 0円 |\n"
}

const fn const_str_w116() -> &'static str {
    // A sequence written and never named.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5円 | a |\n| >5円 | b |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n  exhausted -> held\n\n\
     sequence s(s)\n| k |\n| 3円 |\n"
}

const X_E026: &str = const_str_e026();
const X_E027: &str = const_str_e027();
const X_W116: &str = const_str_w116();

const fn const_str_w115() -> &'static str {
    // `b` is in the enum but no row produces it, and the fold still waits for it.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| - | a |\n\n\
     fold d over xs\n  a -> take_first k\n  b -> next\n  empty -> 0円\n  exhausted -> held\n"
}
const X_W115: &str = const_str_w115();

const X_E020: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\n\
                      elements xs(xs)\n  k(k) : number  range >=0 <=10\n\n\
                      elements ys(ys)\n  m(m) : number  range >=0 <=10\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| n | -> r(r) : bool |\n| - | true |\n";
const X_E021: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\n\
                      elements xs(xs)\n  k(k) : number  range >=0 <=10\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| n | -> r(r) : bool |\n| - | true |\n\n\
                      fold r\n  empty -> false\n";

const X_E028: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\n\
                      elements xs(xs)\n  b(b) : bool\n\n\
                      outputs\n  r(r) : bool\n\n\
                      count h(h) where b  range >=0 <=10\n\n\
                      table j(j)\npolicy unique\n| n | -> r(r) : bool |\n| - | true |\n";
const X_E029: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n  ok(ok) : bool\n\n\
                      elements xs(xs)\n  b(b) : bool\n\n\
                      outputs\n  r(r) : bool\n\n\
                      count h(h) over xs where ok  range >=0 <=10\n\n\
                      table j(j)\npolicy unique\n| h | -> r(r) : bool |\n| - | true |\n";
const X_E030: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\n\
                      elements xs(xs)\n  b(b) : bool\n\n\
                      outputs\n  r(r) : bool\n\n\
                      count h(h) over xs where b\n\n\
                      table j(j)\npolicy unique\n| h | -> r(r) : bool |\n| - | true |\n";
const X_E031: &str = "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
                      elements xs(xs)\n  k(k) : number  range >=0 <=10\n\n\
                      outputs\n  r(r) : number  round down(1)\n\n\
                      table j(j)\npolicy unique\n| k | -> d(d) : v |\n| <=5 | a |\n| >5 | b |\n\n\
                      count h(h) over xs where d = a  range >=0 <=10\n\n\
                      fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0\n  exhausted -> held\n";
const X_E101: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\n\n\
                      inputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : bool |\n\
                      | a | true |\n| b | false |\n";
const X_E102: &str = "rule t(t) v1\n\ninputs\n  w(w) : mass[g]  range >=0g <=10kg\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy first\n| w | -> r(r) : bool |\n\
                      | - | true |\n| <=1000g | false |\n";
const X_E103: &str = "rule t(t) v1\n\ninputs\n  w(w) : mass[g]  range >=0g <=10kg\n  \
                      p(p) : money[円, incl_tax]  range >=0円 <=1万円\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| w | -> r(r) : money[円, incl_tax] |\n| - | 100円 |\n\n\
                      result r = p + w\n";
const X_E104: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : money[円, incl_tax] |\n\
                      | true | 100円 |\n| false | 200円 |\n";
const X_E105: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\n\
                      inputs\n  x(x) : k\n  y(y) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| x | y | -> r(r) : money[円, incl_tax] |\n\
                      | a | - | 100円 |\n| - | true | 200円 |\n| b | false | 300円 |\n";
const X_E106: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round up(10円)\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : money[円, incl_tax] |\n\
                      | true | 1451円 |\n| false | 1000円 |\n";
const X_E107: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : money[円, incl_tax] |\n\
                      | true | 100円 |\n| false | 200円 |\n\n\
                      examples\n| x | -> r |\n| true | 200円 |\n";
const X_E108: &str = "rule t(t) v1\n\n\
                      inputs\n  p(p) : money[円, incl_tax]  range >=0円 <=100000000000000000円\n  \
                      q(q) : rate[step 1%]  range >=0% <=100%\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      define off(off) : money[円, incl_tax] = p × q\n\n\
                      table j(j)\npolicy unique\n| off | -> r(r) : money[円, incl_tax] |\n| - | 0円 |\n";
const X_E109: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\n\n\
                      inputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : bool |\n\
                      | a | true |\n| b | false |\n| c | true |\n";
const X_E110: &str = "rule t(t) v1\n\ninputs\n  s(s) : string\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| s | -> r(r) : bool |\n| \"a\" | true |\n";
const X_E111: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\n\
                      outputs\n  ok(ok) : bool\n  fee(fee) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| x | -> ok(ok) : bool | fee(fee) : money[円, incl_tax] |\n\
                      | true | true | 100円 |\n| false | false | 0円 |\n\n\
                      examples\n| x | -> ok |\n| true | true |\n";
const X_E112: &str = "rule t(t) v1\n\n\
                      inputs\n  a(a) : money[円, incl_tax]  range >=0円 <=100万円\n  \
                      b(b) : money[円, incl_tax]  range >=0円 <=100万円\n\n\
                      outputs\n  r(r) : bool\n\n\
                      derive gap(gap) : money[円, incl_tax] = a - b  range >=0円 <=100万円\n\n\
                      table j(j)\npolicy unique\n| gap | -> r(r) : bool |\n\
                      | <=0円 | false |\n| >0円 | true |\n";
const X_E113: &str = "rule t(t) v1\n\n\
                      inputs\n  a(a) : money[円, incl_tax]  range >=0円 <=100万円\n  \
                      b(b) : money[円, incl_tax]  range >=0円 <=100万円\n\n\
                      outputs\n  r(r) : bool\n\n\
                      define bigger(bigger) : bool = a >= b\n\n\
                      table j(j)\npolicy unique\n| bigger | -> r(r) : bool |\n\
                      | true | true |\n| false | false |\n";
const X_E114: &str = "rule t(t) v1\n\ninputs\n  r(r) : rate[step 1%]  range >=0% <=100%\n\n\
                      outputs\n  o(o) : bool\n\n\
                      table j(j)\npolicy first\n| r | -> o(o) : bool |\n\
                      | <=0.5% | true |\n| - | false |\n";
const X_E115: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=100\n  \
                      d(d) : number  range >=1 <=100\n\n\
                      outputs\n  o(o) : bool\n\n\
                      define r(r) : number = n \u{00f7} d\n\n\
                      table j(j)\npolicy first\n| r | -> o(o) : bool |\n| - | true |\n";

const X_W105: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\n\
                      inputs\n  x(x) : k\n  y(y) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy first\n| x | y | -> r(r) : money[円, incl_tax] |\n\
                      | a | - | 100円 |\n| - | true | 200円 |\n| - | - | 300円 |\n";
const X_W110: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\n\
                      inputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy first\n| x | -> r(r) : bool |\n\
                      | a | true |\n| b | false |\n";
const X_W111: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n  w(w) : mass[g]  range >=0g <=10kg\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : bool |\n\
                      | true | true |\n| false | false |\n";
const X_W114: &str = "rule t(t) v1\n\n\
                      inputs\n  total(total) : money[円, incl_tax]  range >=0円 <=100万円\n  \
                      d1(d1) : money[円, incl_tax]  range >=0円 <=100万円\n  \
                      d2(d2) : money[円, incl_tax]  range >=0円 <=100万円\n\n\
                      outputs\n  r(r) : bool\n\n\
                      derive restA(rest_a) : money[円, incl_tax] = total - d1  range >=-100万円 <=100万円\n\
                      derive restB(rest_b) : money[円, incl_tax] = total - d1 - d2  range >=-200万円 <=100万円\n\n\
                      table j(j)\npolicy unique\n| restA | restB | -> r(r) : bool |\n\
                      | <=1000円 | - | false |\n| >1000円 | <3980円 | false |\n\
                      | >1000円 | >=3980円 | true |\n| <=1000円 | >=3980円 | true |\n";

// ── The ledger ───────────────────────────────────────────────────────────

/// Every code, in ledger order: syntax and names first (E001–E013), then the table checks
/// (E101–E113), then the warnings. There are no vacant numbers left.
pub fn ledger() -> Vec<Entry> {
    vec![
        err(
            "E001",
            tr!("文字列が閉じていません", "Unterminated string"),
            tr!(
                "`\"` で開いた文字列が、その行のうちに閉じていないとき。セルの中にも `description` にも改行は書けません。",
                "A string opened with `\"` is not closed on the same line. Neither a cell nor a `description` may contain a line break."
            ),
            tr!(
                "その行のうちに `\"` を閉じてください。長い説明は一行に収めるか、`#` のコメントに移します。",
                "Close the `\"` on the same line. A long description either fits on one line or moves into a `#` comment."
            ),
            X_E001,
            &["E002"],
        ),
        err(
            "E002",
            tr!("読めない文字があります", "Unreadable character"),
            tr!(
                "名前の位置に、文字でも `_` でもない文字（記号や制御文字）があるとき。打ち間違いが黙って名前になるのを防ぐための検査です。",
                "A character that is neither a letter nor `_` appears where a name is expected. The check exists so that a typo does not quietly become a name."
            ),
            tr!(
                "その文字を消してください。名前は文字か `_` で始まります（`@x(x)` なら `x(x)`）。",
                "Delete the character. A name starts with a letter or `_` (`@x(x)` becomes `x(x)`)."
            ),
            X_E002,
            &["E001", "E009"],
        ),
        err(
            "E003",
            tr!("ファイルが `rule` の行で始まっていません", "The file does not start with a `rule` line"),
            tr!(
                "一つの `.rule` は一つの規則で、先頭行が規則名と版です。空行とコメントより前に他の宣言があるとき。",
                "One `.rule` is one rule, and its first line carries the name and the version. Anything else comes before it."
            ),
            tr!(
                "先頭に `rule 規則名(alias) v1` の一行を足してください。",
                "Add `rule 規則名(alias) v1` as the first line."
            ),
            X_E003,
            &["E004", "E011"],
        ),
        err(
            "E004",
            tr!("行の先頭に語がありません", "The line does not start with a word"),
            tr!(
                "表でもコメントでも空行でもない行が、記号で始まっているとき。この構文は行指向なので、行の先頭の語が何の宣言かを決めます。",
                "A line that is neither a table row, a comment nor blank starts with a symbol. The syntax is line-oriented: the first word of a line decides what is being declared."
            ),
            tr!(
                "行頭に宣言の語を書いてください（`{}`）。表の行なら `|` で始めます。",
                "Start the line with a declaring word (`{}`). A table row starts with `|`.",
                crate::kw::line_heads()
            ),
            X_E004,
            &["E003", "E005"],
        ),
        err(
            "E005",
            tr!("この位置に書けない語です", "A word that cannot appear at this position"),
            tr!(
                "行頭の語が語彙にないとき。語彙には同義の綴りがなく、英語の一種類だけです（§1.1）。",
                "The word at the head of the line is not in the vocabulary. The vocabulary has no synonyms: one English spelling each (§1.1)."
            ),
            tr!(
                "`{}` のどれかに直してください。業務の語は名前とセルの中にだけ書きます。",
                "Correct it to one of `{}`. Business words belong in names and cells, not at the head of a line.",
                crate::kw::line_heads()
            ),
            X_E005,
            &["E004", "E009"],
        ),
        err(
            "E006",
            tr!("宣言に `=` がありません", "The declaration has no `=`"),
            tr!(
                "`enum` `group` `derive` `define` `result` は、名前と中身を `=` で分けます。その `=` が無いとき。",
                "`enum`, `group`, `derive`, `define` and `result` separate the name from the body with `=`. It is missing."
            ),
            tr!(
                "`=` を入れてください。例: `enum k(k) = a(a) | b(b)`。",
                "Insert the `=`, e.g. `enum k(k) = a(a) | b(b)`."
            ),
            X_E006,
            &["E005"],
        ),
        err(
            "E007",
            tr!("そういう方式はありません", "No such policy"),
            tr!(
                "`policy` の後ろが {} 以外のとき。DMN の Any / Priority / Collect は採っていません（§4）。",
                "The word after `policy` is not {}. DMN's Any, Priority and Collect are not adopted (§4).",
                crate::kw::policies()
            ),
            tr!(
                "`policy unique`（重なりはすべてエラー）か `policy first`（先に書いた行が勝つ）に直してください。",
                "Write either `policy unique` (every overlap is an error) or `policy first` (the earlier row wins)."
            ),
            X_E007,
            &["W110", "E105", "W105"],
        ),
        err(
            "E008",
            tr!("空のセルがあります", "Empty cell"),
            tr!(
                "表のセルが空白だけのとき。**空欄は書き忘れと区別がつかない**ので構文エラーにしています（§3）。",
                "A cell of a table holds only whitespace. **A blank is indistinguishable from a forgotten entry**, so it is a syntax error (§3)."
            ),
            tr!(
                "任意の値のつもりなら `-` と書いてください。値を書き忘れていたなら、その値を書きます。",
                "If any value is meant, write `-`. If a value was forgotten, write the value."
            ),
            X_E008,
            &["E101"],
        ),
        err(
            "E009",
            tr!("宣言の名前が予約語と衝突しています", "A declared name collides with a reserved word"),
            tr!(
                "宣言した名前（や別名）が語彙の語と同じとき。行指向のパーサがその行をセクションの始まりと読んで、宣言を黙って捨てる事故を防ぎます。",
                "A declared name (or alias) is the same as a word of the vocabulary. Otherwise the line-oriented parser reads the line as the start of a section and drops the declaration silently."
            ),
            tr!(
                "名前を変えてください（`enum range(kind)` なら `enum 範囲区分(range_kind)`）。予約語は `src/kw.rs` の一枚の表で決まっています。",
                "Rename it (`enum range(kind)` becomes `enum 範囲区分(range_kind)`). The reserved words are fixed by the one table in `src/kw.rs`."
            ),
            X_E009,
            &["E005", "E011"],
        ),
        err(
            "E010",
            tr!("`..` を使った範囲記法は書けません", "The `..` range notation is not allowed"),
            tr!(
                "セルに `0g..1000g` のような `..` があるとき。「1000g まで」が両端を含むのか含まないのかが、書いてある字から読めないためです（§3.1）。",
                "A cell contains `..`, as in `0g..1000g`. Whether \"up to 1000g\" includes the endpoint cannot be read off the text (§3.1)."
            ),
            tr!(
                "比較演算子で書き直してください。`0g..1000g` は `<=1000g` か `<1000g` のどちらかです。両端を決めたいなら `>=0g <=1000g` と並記します。",
                "Rewrite it with comparison operators: `0g..1000g` is either `<=1000g` or `<1000g`. To fix both ends, write `>=0g <=1000g`."
            ),
            X_E010,
            &["E105", "E101"],
        ),
        err(
            "E011",
            tr!("公開される名前に ASCII の別名がありません", "A public name has no ASCII alias"),
            tr!(
                "規則名・入力・出力に、**ASCII でない名前**が付いていて括弧の中の別名も無いとき。別名は生成コードの公開名になります（漢字は大文字を持てず、Go の公開識別子になれません）。名前がもとから ASCII なら、それ自身が公開名になるので別名は要りません。",
                "The rule name, an input or an output has a **non-ASCII name** and no alias in parentheses. The alias becomes the public name in the generated code (a kanji has no uppercase and cannot begin an exported Go identifier). A name that is already ASCII is its own public name and needs no alias."
            ),
            tr!(
                "括弧で別名を足してください（`重量 : bool` なら `重量(weight) : bool`）。導出・定義・グループ・表の別名は任意で、書けば生成コードがその名前を使い、書かなければ宣言した名前をそのまま使います。",
                "Add the alias in parentheses (`重量 : bool` becomes `重量(weight) : bool`). On a derived value, a definition, a group or a table the alias is optional: write one and the generated code uses it, leave it out and the declared name is used as it stands."
            ),
            X_E011,
            &["E009", "E012"],
        ),
        err(
            "E012",
            tr!("宣言されていない名前です", "The name is not declared"),
            tr!(
                "表の列や式が、どこにも宣言されていない名前を指しているとき。前方参照は書けないので、名前はそれを使う行より上で宣言します。",
                "A column or an expression names something that is declared nowhere. There is no forward reference: a name is declared above the line that uses it."
            ),
            tr!(
                "綴りを宣言に合わせるか、その入力・導出・定義を上に宣言してください。",
                "Match the spelling to the declaration, or declare the input, derived value or definition above."
            ),
            X_E012,
            &["E011", "E013"],
        ),
        err(
            "E013",
            tr!("取込先がありません", "No such import"),
            tr!(
                "`import` の先が組み込みの名前空間に無いとき。いまあるのは `std/都道府県`（47 値）だけです。",
                "The target of `import` is not in the built-in namespace. The only one for now is `std/都道府県` (47 values)."
            ),
            tr!(
                "`import std/都道府県` に直すか、その列挙を `enum` でこのファイルに書いてください。",
                "Correct it to `import std/都道府県`, or declare the enum in this file with `enum`."
            ),
            X_E013,
            &["E012"],
        ),
        err(
            "E014",
            tr!("出力のセルに式は書けません", "An output cell cannot hold an expression"),
            tr!(
                "表の `->` から右のセルに語が二つ以上あるとき。書けるのは値一つか名前一つだけです（§3.2）。読み飛ばして最初の語だけを採ると、かけ算が黙って消えた生成コードが出ます。",
                "A cell to the right of `->` holds two or more words. Only one value or one name may be written there (§3.2). Taking just the first word and skipping the rest would emit generated code with the multiplication silently dropped."
            ),
            tr!(
                "計算に名前を付けて `define` の行へ出し、表にはその名前だけを書いてください（`| - | 率割引 |`）。表は分岐だけを持ちます。",
                "Give the calculation a name on a `define` line and leave only that name in the table (`| - | 率割引 |`). A table holds the branching and nothing else."
            ),
            X_E014,
            &["E008", "E012"],
        ),
        err(
            "E015",
            tr!("`result` が書けるのは最初の出力だけです", "`result` can only assemble the first output"),
            tr!(
                "`result` が二つ目以降の出力を名指ししたとき。`result` は最初の出力を組み立てるための書き方で、評価器も生成コードもそこにしか当てません（§1.2）。名指しが効かないまま通っていたので、`number` が `money` の枠に入っても E103 が出ませんでした。",
                "A `result` names an output other than the first. `result` is sugar for the first output, and both the evaluator and the generated code apply it only there (§1.2). The name used to be ignored, so a `number` could land in a `money` slot without an E103."
            ),
            tr!(
                "その出力と同じ名前の `define` を書いてください（`define 付与点(pts) : number = 基本点 × 倍率`）。出力は宣言順に、同じ名前の束縛から取られます。`result` で組み立てたいなら、その出力を `outputs` の先頭へ移します。",
                "Write a `define` of the same name as that output (`define 付与点(pts) : number = 基本点 × 倍率`); outputs are taken, in declaration order, from the binding of their own name. To assemble it with `result` instead, move that output to the top of `outputs`."
            ),
            X_E015,
            &["E016", "E103"],
        ),
        err(
            "E016",
            tr!("`result` は一つしか書けません", "There can be only one `result`"),
            tr!(
                "一つのファイルに `result` の行が二本以上あるとき。組み立てられるのは最初の出力だけなので、二本目は一本目を置き換えるだけになります。以前はそれが黙って起きていました。",
                "A file has more than one `result` line. Only the first output can be assembled, so a second `result` merely replaces the first — which it used to do in silence."
            ),
            tr!(
                "一本だけ残してください。ほかの出力は、その出力と同じ名前の `define` から取ります。",
                "Keep one. The other outputs are taken from a `define` of the same name as the output."
            ),
            X_E016,
            &["E015"],
        ),
        err(
            "E017",
            tr!("`constraint` の形が違います", "A `constraint` is not shaped like this"),
            tr!(
                "`constraint` の行が「入力 比較 入力」になっていないとき。比較が無い、片側が名前一つでない、のどちらかです。",
                "A `constraint` line is not `input comparison input`: either there is no comparison, or a side is not a single name."
            ),
            tr!(
                "`constraint <入力> <= <入力>` の形にしてください。比較は `<=` `<` `>=` `>` の四つです。`A = B` を言いたいなら、`A <= B` と `A >= B` の二行に分けます。",
                "Write `constraint <input> <= <input>`; the comparisons are `<=`, `<`, `>=` and `>`. For `A = B`, write the two lines `A <= B` and `A >= B`."
            ),
            X_E017,
            &["E018"],
        ),
        err(
            "E018",
            tr!("`constraint` は入力どうしの関係です", "A `constraint` relates two inputs"),
            tr!(
                "`constraint` の片側が入力でないか、順序の無い型のとき。制約は「呼び出し側が渡す値の組み合わせのうち、どれが起きるか」を言うものなので、両側とも `inputs` の名前で、金額・数量・率・number・日付のいずれかです。",
                "A side of a `constraint` is not an input, or has a type with no order. A constraint says which combinations of the values the caller passes can happen, so both sides name something in `inputs`, and each is money, a quantity, a rate, a number or a date."
            ),
            tr!(
                "両側を `inputs` の名前にしてください。導出や定義は入力から計算されるので、関係は元の入力どうしで書きます。列挙や真偽に大小はありません。",
                "Name inputs on both sides. A derived or defined value is computed from inputs, so write the relation between those inputs; an enum and a boolean have no order."
            ),
            X_E018,
            &["E017", "W111"],
        ),
        err(
            "E019",
            tr!("例が制約を破っています", "An example breaks a constraint"),
            tr!(
                "例の入力が `constraint` を満たしていないとき。制約は「この組み合わせは起きない」という宣言で、完全性の検査はそれを信じて、その組み合わせには行を要求していません。生成コードもその入力を入口で断ります。答えを主張できない入力です。",
                "An example's inputs do not satisfy a `constraint`. The constraint declares that the combination does not happen, the completeness check believed it and demanded no row there, and the generated code refuses that input at the door. It is not an input an answer can be claimed for."
            ),
            tr!(
                "例の値を直してください。その組み合わせが本当に起きるなら、制約のほうが間違っているので消します。",
                "Correct the example's values — or, if that combination really does happen, the constraint is what is wrong and it goes."
            ),
            X_E019,
            &["E017", "E018", "E101"],
        ),
        err(
            "E020",
            tr!("`elements` の宣言が正しくありません", "The `elements` declaration is not right"),
            tr!(
                "`elements` に名前が無いか、二本あるとき。規則がたどる並びは一つで、その一要素ぶんの欄をそこに書きます（§15.56）。",
                "An `elements` line has no name, or there are two of them. A rule walks one sequence, and the fields of one of its elements are declared there (§15.56)."
            ),
            tr!(
                "`elements 運賃行(fee_rows)` の形にして、続く行に一要素ぶんの欄を `inputs` と同じように書いてください。並びが二つ要るなら、それは別の規則です。",
                "Write `elements 運賃行(fee_rows)`, and the fields of one element under it, declared the way `inputs` are. Two sequences mean two rules."
            ),
            X_E020,
            &["E021"],
        ),
        err(
            "E021",
            tr!("`fold` の書き方が正しくありません", "The `fold` is not written correctly"),
            tr!(
                "`fold <判定の列> over <並びの名前>` になっていないか、判定の行き先が `next` `stop` `stop with <値>` `take_unique <値>` `take_first <値>` `keep_max <値> by <鍵>` のどれでもないか、畳もうとしている列が列挙でないとき。",
                "The heading is not `fold <verdict column> over <sequence>`, or an arm is not one of `next`, `stop`, `stop with <value>`, `take_unique <value>`, `take_first <value>`, `keep_max <value> by <key>`, or the column being folded is not an enum."
            ),
            tr!(
                "見出しと行き先を上の形に直してください。`take` とだけ書くことはできません。**一件だけ採るのか、最初の一件を採るのか**は、書く人が選ぶことだからです（§15.56）。",
                "Correct the heading and the arms. A bare `take` cannot be written: whether **one and only one** element may be taken, or the first of several, is for the author to choose (§15.56)."
            ),
            X_E021,
            &["E020", "E022", "E023", "E024"],
        ),
        err(
            "E022",
            tr!("要素がゼロ件のときの答えが宣言されていません", "The answer for a sequence with no elements is not declared"),
            tr!(
                "`fold` に `empty -> <値>` が無いとき。空の並びは必ず来ます。手で書いたループがいちばんよく落とすのがこの場合で、たいていは最初の要素をそのまま読んで落ちます。",
                "A `fold` has no `empty -> <value>`. An empty sequence always turns up, and it is the case a hand-written loop most often forgets — usually by reading the first element and falling over."
            ),
            tr!(
                "`empty -> <値>` を足してください。何を返すかは業務の判断で、道具が決められることではありません。",
                "Add `empty -> <value>`. What to answer is a business decision, and not one the tool can make."
            ),
            X_E022,
            &["E023", "E024"],
        ),
        err(
            "E023",
            tr!("最後まで見終えたときの答えが宣言されていません", "The answer for a walk that reached the end is not declared"),
            tr!(
                "`fold` に `exhausted -> <値>` が無いとき。どの要素も打ち切らずに並びが尽きた場合の答えです。保持していた暫定の値をそのまま返すつもりでも、それは書いて初めて決まります。",
                "A `fold` has no `exhausted -> <value>`: the answer when the sequence ran out and no element ended the walk. Answering with the value that was held is a choice, and it is made by writing it."
            ),
            tr!(
                "`exhausted -> <値>` を足してください。保持しているものを返すなら `exhausted -> held` です。",
                "Add `exhausted -> <value>`; to answer with what is held, that is `exhausted -> held`."
            ),
            X_E023,
            &["E022", "E024"],
        ),
        err(
            "E024",
            tr!("行き先の無い判定があります", "Some verdict has no arm"),
            tr!(
                "表が出しうる判定のどれかに、`fold` の行き先が無いとき。その判定の要素が来たら、次にどうするかが決まっていません。表の完全性と同じ検査を、畳み込みの側に当てたものです（§15.56）。",
                "A verdict the table can produce has no arm in the `fold`: when an element lands on it, the walk has no move. It is the table's own completeness check, applied to the fold (§15.56)."
            ),
            tr!(
                "行き先を足すか、表がその値を出さないようにしてください。逆に、どの要素も辿り着けない判定に行き先があるときは W115 が出ます。",
                "Add the arm, or stop the table producing that value. The other direction — an arm for a verdict nothing can reach — is W115."
            ),
            X_E024,
            &["E022", "E023", "W115"],
        ),
        err(
            "E025",
            tr!("例に並びの欄がありません", "The examples have no column for the sequence"),
            tr!(
                "並びをたどる規則に `examples` があるのに、`elements` の名前の欄が見出しに無いとき。どの並びをたどるかが決まっていない例は、答えの決まっていない例です（§15.56）。",
                "A rule that walks a sequence has `examples`, but the header has no column named after its `elements`. An example that does not say which sequence it walks is an example with no answer (§15.56)."
            ),
            tr!(
                "`sequence <名前>` で並びを書き、例の見出しに並びの欄を足して、その名前をセルに書いてください。行がゼロ本の `sequence` は、要素ゼロ件の例になります。",
                "Write the list with `sequence <name>`, add a column for the sequence to the examples header, and name it in the cell. A `sequence` with no rows is the example for a sequence with nothing in it."
            ),
            X_E025,
            &["E026", "E027"],
        ),
        err(
            "E026",
            tr!("`sequence` の書き方が正しくありません", "The `sequence` is not written correctly"),
            tr!(
                "`sequence` の欄が `elements` の欄とそろっていないとき——余分な欄がある、欄が足りない、`->` がある、たどる並びそのものが無い、同じ名前が二つある、セルが値でない（範囲や `-` が書いてある）。",
                "The columns of a `sequence` do not line up with the fields of `elements`: a column that is not a field, a field left out, a `->`, no sequence to be a list of, two blocks with the same name, or a cell that is not a value (a range or a `-`)."
            ),
            tr!(
                "`elements` の欄をそのまま見出しにして、一行に一件ぶんの値を書いてください。これは表ではなく、実際に渡す値の並びです。",
                "Make the header the fields of `elements` as they are, and write one element's values per row. This is not a table: it is the list of values as they would really be passed."
            ),
            X_E026,
            &["E025", "E020"],
        ),
        err(
            "E027",
            tr!("例が指す並びがありません", "The example names a sequence that is not there"),
            tr!(
                "例の並びの欄に書かれた名前の `sequence` が無いとき、またはその欄に名前でないもの（数や範囲）が書かれているとき。",
                "The cell in the sequence column names a `sequence` that is not declared, or holds something that is not a name at all."
            ),
            tr!(
                "その名前で `sequence` を書くか、セルを、書いてある `sequence` の名前に直してください。",
                "Write a `sequence` under that name, or correct the cell to one that is written."
            ),
            X_E027,
            &["E025", "E026"],
        ),
        err(
            "E028",
            tr!("`count` の書き方が正しくありません", "The `count` is not written correctly"),
            tr!(
                "`count <名前>(<別名>) over <並びの名前> where <列> = <値>` になっていないとき。`over` が無い、`where` が無い、指した並びが `elements` で宣言されていない、`=` の右に値が無い（§15.58）。",
                "The line is not `count <name>(<alias>) over <sequence> where <column> = <value>`: no `over`, no `where`, a sequence that `elements` does not declare, or an `=` with nothing on its right (§15.58)."
            ),
            tr!(
                "上の形に直してください。`= <値>` は、真偽の列を数えるときだけ省けます。",
                "Write it in that shape. The `= <value>` may be left out only for a bool column."
            ),
            X_E028,
            &["E029", "E030", "E020"],
        ),
        err(
            "E029",
            tr!("この列は数えられません", "This column cannot be counted"),
            tr!(
                "`where` が指す列が、要素ごとに決まる値でないとき（入力や導出は一件の呼び出しに一つしかないので、数えても 0 か 1 です）。値が有限の集合でないとき。書いた値がその列挙にないとき。列挙の列なのに `= <値>` が無いとき（§15.58）。",
                "The column `where` names is not a value of one element (an input or a derived value is one per call, so counting it could only answer 0 or 1); or its values are not a closed set; or the value written is not one of that enum\'s; or an enum column was given no `= <value>` (§15.58)."
            ),
            tr!(
                "要素の欄か、要素ごとの表が出した列を指してください。判定を表に書けば、その分類そのものも完全性の検査に掛かります。",
                "Name a field of an element, or a column a per-element table produces. Writing the classification as a table is what puts the classification itself under the completeness check."
            ),
            X_E029,
            &["E028", "E012"],
        ),
        err(
            "E030",
            tr!("`count` に範囲が要ります", "A `count` needs a range"),
            tr!(
                "`count` の行に `range >=0 <=<上限>` が無いか、上限が無いか、下限が負のとき。範囲は二つの意味を持ちます——数えた結果を列に使ったときに完全性の検査が見る全体集合と、**並びの長さの上限**です（§15.58）。",
                "The `count` line has no `range >=0 <=<max>`, or no upper bound, or a negative lower one. The range means two things: the universe the completeness check quantifies over once the count is a column, and **the cap on the sequence** (§15.58)."
            ),
            tr!(
                "`range >=0 <=100` の形で書いてください。生成コードは、この上限より長い並びを入口で断ります。数値の入力と同じで、宣言の外は黙って通しません。",
                "Write it as `range >=0 <=100`. The generated code refuses a longer sequence at the door, the way it refuses a number outside its range."
            ),
            X_E030,
            &["E028", "E112"],
        ),
        err(
            "E031",
            tr!("`fold` と `count` は一緒に書けません", "A rule cannot have both a `fold` and a `count`"),
            tr!(
                "一つの規則に `fold` と `count` の両方があるとき。どちらも同じ並びの終わり方で、`fold` は途中で打ち切れるので、止まった歩きの数え上げが何を意味するかが決まりません（§15.58）。",
                "One rule has both. They are two endings for the same walk, and a `fold` can stop partway: what a count means on a walk that stopped is not decided (§15.58)."
            ),
            tr!(
                "数えるなら `fold` を消して、数えた結果を表で判定してください。畳むなら `count` を消してください。",
                "To count, drop the `fold` and let a table judge the count. To fold, drop the `count`."
            ),
            X_E031,
            &["E021", "E029"],
        ),
        err(
            "E101",
            tr!("完全性の欠落: どの行にも当てはまらない入力があります", "Completeness gap: some input matches no row"),
            tr!(
                "行を全部合わせても、宣言した範囲の入力を覆いきれていないとき。完全性は宣言で外せず、常に必須です（§4）。当てはまらない入力の具体例が必ず付きます。",
                "The union of the rows does not cover the declared input space. Completeness cannot be waived and is always required (§4). A concrete input that matches no row is always attached."
            ),
            tr!(
                "それを起こす入力に当てはまる行を足してください。列挙の値が増えたのが原因なら、その値の行か、全部を受ける `-` の行を足します。値に専用の行が要らないなら、列挙の宣言に `default` を付けます。",
                "Add a row that matches the witness. If a new enum value caused it, add a row for that value or a `-` row that catches everything. If the value needs no row of its own, mark it `default` in the enum declaration."
            ),
            X_E101,
            &["E102", "E105", "W111"],
        ),
        err(
            "E102",
            tr!("どの入力にも当てはまらない行があります", "Unreachable row: the row never matches"),
            tr!(
                "先行する行にすべて覆われているか、上流の表が決して出さない値を名指ししているとき。形は二つあり、文面が原因を書き分けます。",
                "Every input the row would take is already taken by an earlier row, or the row names a value that the upstream table never produces. The two forms are told apart in the wording."
            ),
            tr!(
                "その行が新しい仕様なら、覆っている行より上へ移してください。不要なら削除します。上流が出さない値を指しているなら、上流の表にその値を出す行を足すか、この行を消します。",
                "If the row is the newer intent, move it above the row that covers it. If it is dead, delete it. If it names a value the upstream never emits, either add a row upstream that emits it, or delete this row."
            ),
            X_E102,
            &["E101", "W105", "W110"],
        ),
        err(
            "E103",
            tr!("単位の混同: 型の違う値を混ぜています", "Unit mismatch: values of different types are being mixed"),
            tr!(
                "式やセルで、単位・通貨・税区分の違う値を足したり比べたりしているとき。`money[円, incl_tax]` と `money[円, excl_tax]` も別物です（§2.3）。",
                "An expression or a cell adds or compares values whose unit, currency or tax flag differ. `money[円, incl_tax]` and `money[円, excl_tax]` are different types too (§2.3)."
            ),
            tr!(
                "混ぜている片方を表に移してください。「重量に応じた加算料金」なら `table 重量加算 | 重量 | -> 加算額 : money[円, incl_tax] |` の形です。税の変換も、式ではなく表として書きます。",
                "Move one side into a table. \"A surcharge that depends on weight\" is `table 重量加算 | 重量 | -> 加算額 : money[円, incl_tax] |`. A tax conversion is also written as a table, never as a formula."
            ),
            X_E103,
            &["E108", "E112"],
        ),
        err(
            "E104",
            tr!("数値の出力に丸めの宣言がありません", "A numeric output declares no rounding"),
            tr!(
                "数量・金額・率の出力に `round` が無いとき。端数がどう決まるかを宣言しないと、生成コードが黙って決めてしまいます。式が端数を生む場合は、丸め方で円がいくら動くかを数字で見せます。",
                "A quantity, money or rate output has no `round`. Unless the fraction is declared, the generated code settles it silently. When the expression can produce a fraction, the message shows in yen how far the choice moves the answer."
            ),
            tr!(
                "出力の宣言に丸めを書いてください。例: `round up(10円)`。向きは五種（`up` `down` `half_up` `half_down` `half_even`）で、負の側まで固定されています（§7.3）。",
                "Add rounding to the output declaration, e.g. `round up(10円)`. There are five directions (`up`, `down`, `half_up`, `half_down`, `half_even`), pinned down for negative values as well (§7.3)."
            ),
            X_E104,
            &["E106", "E103"],
        ),
        err(
            "E105",
            tr!("行の重なり: 同じ入力が二つ以上の行に当てはまります", "Overlapping rows: the same input matches two or more rows"),
            tr!(
                "`policy unique` の表で、両方に当てはまる入力を実際に構成できたとき。構成できなかった重なりは W114 に落ちます。",
                "In a `policy unique` table, an input matching both rows was actually constructed. An overlap that could not be constructed falls to W114 instead."
            ),
            tr!(
                "出力が違うなら、どちらが正しいか決めて行を直してください。順序に意味を持たせたいなら `policy first` を宣言します。出力まで同じなら、片方を削ります。",
                "If the outputs differ, decide which is right and fix the rows; to let the order decide, declare `policy first`. If even the outputs agree, delete one of the rows."
            ),
            X_E105,
            &["W105", "W114", "E102"],
        ),
        err(
            "E106",
            tr!("出力のリテラルが丸めの刻みに載っていません", "An output literal is not on the rounding grid"),
            tr!(
                "出力セルに書かれたリテラルが、宣言した丸めの刻みの倍数でないとき。`round up(10円)` の表に `1451円` があるような、桁の打ち間違いをここで落とします（§7.2）。",
                "A literal in an output cell is not a multiple of the declared rounding grid. This is where a mistyped digit — `1451円` in a table rounded `up(10円)` — is stopped (§7.2)."
            ),
            tr!(
                "リテラルを刻みに載せてください（`1451円` は `1450円` か `1460円`）。その額が本当に正しいなら、丸めの刻みのほうを直します。",
                "Put the literal on the grid (`1451円` becomes `1450円` or `1460円`). If the amount really is right, change the grid instead."
            ),
            X_E106,
            &["E104"],
        ),
        err(
            "E107",
            tr!("例の期待値と一致しません", "An example does not match"),
            tr!(
                "`examples` の行を参照評価器で走らせた結果が、書かれた期待値と違うとき。**どの表のどの行に当てはまったか**が付きます。`examples` は実行される仕様です。",
                "Running a row of `examples` through the reference evaluator gives something other than the value written. **Which row of which table fired** is attached. `examples` is an executable specification."
            ),
            tr!(
                "表が正しいなら期待値を直してください。期待値が業務の真実なら、当てはまった行のほうを直します。どちらを直すかは、出典（規約・Excel・旧実装）が決めます。",
                "If the table is right, fix the expected value. If the expected value is the business truth, fix the row that fired. Which one to fix is settled by the source: the written rule, the spreadsheet, or the legacy implementation."
            ),
            X_E107,
            &["E111", "E105"],
        ),
        err(
            "E108",
            tr!("中間値が int64 に収まることを証明できません", "Cannot prove an intermediate value fits in int64"),
            tr!(
                "宣言した範囲と刻みから計算した「実際に取りうる値」が、int64 を超えるとき。率の刻みが 1% なら格納される整数は 100 倍になります。",
                "The reachable interval computed from the declared ranges and steps exceeds int64. With a rate step of 1%, the stored integer is 100 times the value."
            ),
            tr!(
                "入力の範囲を狭めるか、途中に丸めを一つ入れてください。どこで丸めるかは円が動く業務の判断なので、道具は勝手に決めません（§7.1）。",
                "Narrow the input ranges, or insert one rounding step along the way. Where to round is a business decision that moves yen, so the tool does not decide it (§7.1)."
            ),
            X_E108,
            &["E112", "E103"],
        ),
        err(
            "E109",
            tr!("検査の予算を超えたので、完全性を証明できませんでした", "The check exceeded its budget, so completeness could not be proven"),
            tr!(
                "領域検査が訪れたノード数が `--budget` を超えたとき。**証明できなかったことを緑にはしない**ので、警告ではなくエラーです。",
                "The region check visited more nodes than `--budget` allows. Failing to prove something is never green here, so this is an error and not a warning."
            ),
            tr!(
                "表を分けて列の数を減らすか、`--budget` を上げてください。列の積が効くので、一つの表に列を積むより、表を一列につないでいくほうが安く済みます（§5.1）。",
                "Split the table to reduce the number of columns, or raise `--budget`. The cost is the product of the columns, so chaining tables in a linear pipeline is cheaper than piling columns into one table (§5.1)."
            ),
            X_E109,
            &["E101", "W114"],
        )
        .with_budget(1),
        err(
            "E110",
            tr!("検査できない型の列があります", "A column has a type the check cannot handle"),
            tr!(
                "その列の型を、区画の計算ができる形に落とせないとき。**これが出たら rulec 自身のバグです。** 表の検査が黙って素通りするのを防ぐための内部の検査で、日付と optional で二度起きた同じ種類の事故を、まとめて塞いだものです（§6.3）。",
                "The column's type cannot be lowered into the region IR. **Seeing this is a bug in rulec itself.** It is the internal breakwater that stops a table from being skipped silently, put in after the same accident happened twice, with dates and with optional (§6.3)."
            ),
            tr!(
                "その列を、いま検査できる型（真偽・列挙・数量・金額・率・日付・optional）に直してください。そのうえで報告してください — 素通りより止まるほうが正しいという判断でこの検査があります。",
                "Change the column to a type the check handles today (bool, enum, quantity, money, rate, date, optional). Then report it: this check exists because stopping is better than passing silently."
            ),
            X_E110,
            &["E101", "E105"],
        ),
        err(
            "E111",
            tr!("例に出力の列がありません", "The examples have no column for an output"),
            tr!(
                "`examples` が、宣言した出力の一部しか書いていないとき。実装どうしの照合は、生成物が揃って同じ誤りを持つと緑のままなので、**それを破れるのは人の書いた期待値だけ**です。実際に複数出力の丸めが、どの言語でも揃って抜けたことがあります。",
                "`examples` writes only some of the declared outputs. Agreement across the implementations stays green when they all carry the same mistake, so **only a human-written expectation can break it**. Rounding for multiple outputs really did go missing in every one at once."
            ),
            tr!(
                "欠けている出力の列を `examples` に足してください。出力が二つ以上あるときは、二列目以降に `->` を書いても書かなくても構いません。",
                "Add the missing output column to `examples`. With two or more outputs, writing `->` before the later columns is optional."
            ),
            X_E111,
            &["E107"],
        ),
        err(
            "E112",
            tr!("導出の範囲が、実際に到達しうる値を含んでいません", "The range of a derived value does not contain the values it can reach"),
            tr!(
                "入力の範囲から計算した「実際に取りうる値」が、導出に宣言した `range` からはみ出すとき。範囲が狭いと、完全性検査が実際に起きる値を見ないまま「完全」と答えます。",
                "The interval computed from the input ranges falls outside the `range` declared on the derived value. With too narrow a range, the completeness check answers \"complete\" without ever looking at values that really occur."
            ),
            tr!(
                "文面が示す範囲まで `range` を広げてください（`range >=-110万円 <=100万円` の形で書いてあります）。起こりえない分まで広げても、検査がそこを自分で外すので害はありません。",
                "Widen the `range` to the reachable interval the message states (it is written out, e.g. `range >=-110万円 <=100万円`). Widening past what is reachable costs nothing: the check sifts the infeasible part out."
            ),
            X_E112,
            &["E108", "E101"],
        ),
        err(
            "E113",
            tr!("真偽定義の条件が、書ける二つの形のどちらでもありません", "The condition of a boolean definition is neither of the two allowed forms"),
            tr!(
                "`define … : bool` の条件が、「入力か導出の値ひとつを定数と比べる」形でも、「引き算で差を取れない型どうしの比較」（日付どうしなど）でもないとき。数値どうしを直接比べたときがこれに当たります（§5.3）。",
                "The condition of `define … : bool` is neither one input or derived value compared with a constant, nor a comparison of two values whose difference cannot be subtracted (two dates, say). Comparing two numbers directly is the usual case (§5.3)."
            ),
            tr!(
                "差を導出として宣言してから定数と比べてください。`define bigger : bool = a >= b` は `derive gap(gap) : money[円, incl_tax] = a - b  range …` を足して `| gap | >=0円 |` と書き換えます。そのほうが厳密に解析できます。",
                "Declare the difference as a derived value and compare that against a constant. `define bigger : bool = a >= b` becomes `derive gap(gap) : money[円, incl_tax] = a - b  range …` and the cell `>=0円`. The analysis is exact that way."
            ),
            X_E113,
            &["E112", "E103"],
        ),
        err(
            "E114",
            tr!("セルの値が列の刻みに載っていません", "A cell value does not sit on the column's step"),
            tr!(
                "`rate[step 1%]` の列に `0.5%` のように、宣言した刻みの整数倍でない値が書かれたとき。実行時の値はその刻みの整数一本なので（§2.1）、この値には表し方がありません。",
                "A value that is not a whole number of the declared step is written in the column, such as `0.5%` where the type says `rate[step 1%]`. At runtime the value is one integer count of that step (§2.1), so this one has no representation."
            ),
            tr!(
                "刻みに載る値に直すか、型の刻みを細かくしてください（`rate[step 0.1%]`）。黙って近い刻みに寄せると、表で読める境界と生成コードの境界が食い違います。",
                "Write a value on the step, or declare a finer step (`rate[step 0.1%]`). Quietly moving it to the nearest step would make the boundary on the page differ from the boundary in the generated code."
            ),
            X_E114,
            &["E103", "E106"],
        ),
        err(
            "E115",
            tr!("変数では割れません", "Cannot divide by a variable"),
            tr!(
                "`÷` の右が定数でないとき。割る数は正の整数の定数か、同じ単位の金額・数量の定数だけです（§2.3）。",
                "The right of `÷` is not a constant. A divisor is a positive whole constant, or a constant amount or quantity in the same unit (§2.3)."
            ),
            tr!(
                "割る数が業務のデータなら、率として入力に取るか、定数を引く表として書いてください。刻みが静的に決まらないと生成コードは言語の除算に頼ることになり、Python は −∞ 方向、Go は 0 方向に丸めて答えが食い違います（§7.1）。",
                "If the divisor is business data, take it as a rate input or look the constant up in a table. Without a statically known step the generated code falls back on the language's own division, and Python rounding toward -inf and Go toward zero disagree (§7.1)."
            ),
            X_E115,
            &["E103", "E108"],
        ),
        warn(
            "W105",
            tr!("要確認の隠れ: 先の行が後の行の一部を隠しています", "Shadowing that needs review: an earlier row hides part of a later one"),
            tr!(
                "`policy first` の表で、一部だけ重なっていて出力が違う行の対があるとき。階段状の隠れと、答えが同じ隠れは件数の注記に畳まれ、ここに一覧されるのは要確認の対だけです（§4）。",
                "In a `policy first` table, two rows partially intersect and disagree on the output. Structural shadowing (the staircase) and equivalent shadowing are folded into a count line; only the pairs that need review are listed (§4)."
            ),
            tr!(
                "意図どおりならこのままで構いません（CI の `check --diff-base` は新たに生じた対だけを報告します）。後の行を優先したいなら、その行を先の行より上へ移してください。全対を見るには `--show-shadow` を付けます。",
                "If it is intended, leave it: `check --diff-base` in CI reports only newly created pairs. To let the later row win, move it above the earlier one. `--show-shadow` lists every pair."
            ),
            X_W105,
            &["E105", "W110", "E102"],
        ),
        warn(
            "W110",
            tr!("重なりのない `first` です", "A `first` table with no overlaps"),
            tr!(
                "`policy first` なのに、どの二行も重ならないとき。順序に意味が無いので、`unique` のほうが強い保証になります。",
                "The table is `policy first`, yet no two rows overlap. The order carries no meaning, and `unique` is the stronger guarantee."
            ),
            tr!(
                "`policy unique` に変えてください。並べ替えても意味が変わらないことが、以後の検査で守られます。",
                "Change it to `policy unique`. From then on, the check guarantees that reordering the rows cannot change the meaning."
            ),
            X_W110,
            &["W105", "E105"],
        ),
        warn(
            "W111",
            tr!("使われていない宣言があります", "A declaration is never used"),
            tr!(
                "入力・導出・グループ・列挙の値が、どの表のどのセルにも現れないとき。書き忘れのしるしであることも、意図した契約であることもあります。`import` で持ち込んだ型の値は対象外です。",
                "An input, a derived value, a group or an enum value appears in no cell of any table. It can be the symptom of a forgotten column, or a legitimate contract. Values of an imported type are not checked this way."
            ),
            tr!(
                "入力が範囲の入口検査としてだけ効いているなら、宣言に `contract_only` を付けて黙らせてください。列挙の値が既定行に吸われるのが正しいなら、値の宣言に `default` を付けます。どちらでもないなら、その列を表に足すか、宣言を消します。",
                "If the input serves only as a range check at the entry, mark the declaration `contract_only`. If an enum value is meant to fall through to the default row, mark the value `default`. Otherwise either add the column to a table, or delete the declaration."
            ),
            X_W111,
            &["E101", "E012"],
        ),
        warn(
            "W116",
            tr!("どの例も使っていない `sequence` です", "No example uses this sequence"),
            tr!(
                "`sequence` を書いたのに、どの例もその名前を書いていないとき。並びは例から名指しされて初めて走るので、走っていない並びです（§15.56）。",
                "A `sequence` is written and no example names it. A sequence runs only when an example names it, so this one never runs (§15.56)."
            ),
            tr!(
                "その並びをたどる例を足すか、並びのほうを消してください。書いたのに使っていないのは、たいてい例を書き忘れた跡です。",
                "Add the example that walks it, or drop the sequence. Written and unused is usually the trace of an example left unwritten."
            ),
            X_W116,
            &["E027", "W111"],
        ),
        warn(
            "W115",
            tr!("どの要素もこの判定にはなりません", "No element can land on this verdict"),
            tr!(
                "`fold` にその判定の行き先があるのに、表のどの行もその判定を出さないとき。E024 の裏返しで、こちらは穴ではなく届かない行き先です。書き忘れではなく、表のほうが変わった跡であることが多い（§15.56）。",
                "A `fold` has an arm for a verdict no row produces. It is the other side of E024: not a hole but an arm nothing reaches, and more often the trace of a table that changed than of an arm written by mistake (§15.56)."
            ),
            tr!(
                "表の行を見直すか、その行き先を消してください。どちらが正しいかは表のほうを読まないと決まりません。",
                "Look again at the table's rows, or drop the arm. Which of the two is right is decided by reading the table, not this message."
            ),
            X_W115,
            &["E024"],
        ),
        warn(
            "W114",
            tr!("未確認の重なり: 両方に当てはまる入力が有り得ます", "Unconfirmed overlap: an input may match both rows"),
            tr!(
                "`policy unique` の表で二行が重なりうるが、それを実際に起こす入力を構成できず、実現不能の証明もできなかったとき。導出どうしが入力を共有していると起こります（列ごとに独立に見る検査では、その結びつきが見えません）。",
                "Two rows of a `policy unique` table may overlap, but no input producing that was constructed and infeasibility was not proven either. It happens when derived values share inputs: sifting independent intervals does not see the dependency."
            ),
            tr!(
                "その条件を同時に満たす注文が存在するなら、行を直してください（出力が違うので、当てはまれば矛盾です）。存在しないならこのままで構いません — 生成コードには、万一その条件に当てはまる入力が来たとき黙って先の行を選ばずエラーを返すガードが入ります。",
                "If an order satisfying both conditions can exist, fix the rows: the outputs differ, so a match is a contradiction. If none can exist, leave it — the generated code carries a guard that returns an error rather than silently picking the earlier row."
            ),
            X_W114,
            &["E105", "W105", "E109"],
        ),
    ]
}

pub fn find(code: &str) -> Option<Entry> {
    let up = code.to_ascii_uppercase();
    ledger().into_iter().find(|e| e.code == up)
}

fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// Indent every line of a block by two spaces, so an example reads as one unit in a terminal.
fn indent(s: &str) -> String {
    s.lines().map(|l| if l.is_empty() { String::from("\n") } else { format!("  {l}\n") }).collect()
}

/// `rulec explain <CODE>` — the shape a terminal reads.
pub fn render_text(e: &Entry) -> String {
    let mut o = format!("{}[{}]: {}\n", severity_word(e.severity), e.code, e.title);
    o.push_str(&tr!("\nいつ出るか\n", "\nWhen\n"));
    o.push_str(&indent(&e.when));
    o.push_str(&tr!("\n直し方\n", "\nFix\n"));
    o.push_str(&indent(&e.fix));
    o.push_str(&match e.budget {
        Some(b) => tr!("\n最小の再現（`--budget {b}` で）\n", "\nSmallest reproduction (with `--budget {b}`)\n"),
        None => tr!("\n最小の再現\n", "\nSmallest reproduction\n"),
    });
    o.push_str(&indent(e.example));
    if !e.related.is_empty() {
        o.push_str(&tr!("\n関係するコード: {}\n", "\nRelated codes: {}\n", e.related.join(" ")));
    }
    o
}

/// `rulec explain <CODE> --format markdown`.
pub fn render_markdown(e: &Entry) -> String {
    // The heading is the code alone, so that its anchor is `#e101` both on
    // GitHub and on the rendered site — the `related` links below point at it.
    let mut o = format!("## {}\n\n", e.code);
    o.push_str(&format!("`{}` — **{}**\n\n", severity_word(e.severity), e.title));
    o.push_str(&tr!("**いつ出るか。** {}\n\n", "**When.** {}\n\n", e.when));
    o.push_str(&tr!("**直し方。** {}\n\n", "**Fix.** {}\n\n", e.fix));
    o.push_str(&match e.budget {
        Some(b) => tr!("**最小の再現**（`--budget {b}` で）:\n\n", "**Smallest reproduction** (with `--budget {b}`):\n\n"),
        None => tr!("**最小の再現**:\n\n", "**Smallest reproduction**:\n\n"),
    });
    // Tagged, so that the site colours it; a reader of the plain file sees the tag and
    // nothing else changes.
    o.push_str("```rule\n");
    o.push_str(e.example);
    o.push_str("```\n");
    if !e.related.is_empty() {
        let links: Vec<String> =
            e.related.iter().map(|c| format!("[{c}](#{})", c.to_ascii_lowercase())).collect();
        o.push_str(&tr!("\n関係するコード: {}\n", "\nRelated codes: {}\n", links.join(", ")));
    }
    o
}

/// `rulec explain <CODE> --format json`. Language independent in its keys; the prose
/// fields follow `--lang`.
pub fn render_json(e: &Entry) -> String {
    json::Obj::new()
        .str("code", e.code)
        .str("severity", severity_word(e.severity))
        .str("title", &e.title)
        .str("when", &e.when)
        .str("fix", &e.fix)
        .str("example", e.example)
        .opt_raw("budget", e.budget.map(|b| b.to_string()))
        .raw("related", json::strs(e.related))
        .str("lang", crate::i18n::current().code())
        .finish()
}

/// `rulec explain --all --format markdown`. This **is** `docs/codes.md`; a test holds the
/// checked-in file to it, the same way `gen --check` holds the generated code.
pub fn markdown_all() -> String {
    let all = ledger();
    let mut o = tr!(
        "<!-- `rulec explain --all --format markdown --lang ja` の出力です。手で編集しないでください。 -->\n\n",
        "<!-- Output of `rulec explain --all --format markdown --lang en`. Do not edit by hand. -->\n\n"
    );
    o.push_str(&tr!("# rulec の診断\n\n", "# rulec diagnostics\n\n"));
    o.push_str(&tr!(
        "rulec が出しうるコードの全部と、いつ出るか、どう直すか。コードと JSON の形は安定 API で、文面だけが良くなります（DESIGN §11 原則 5）。一件だけ読むには `rulec explain E101`。\n\n",
        "Every code rulec can print, what makes it appear, and how to fix it. The code and the JSON shape are a stable API; only the prose improves (DESIGN §11 principle 5). For one of them: `rulec explain E101`.\n\n"
    ));
    o.push_str(&tr!(
        "| コード | 種別 | 見出し |\n|---|---|---|\n",
        "| Code | Severity | Title |\n|---|---|---|\n"
    ));
    for e in &all {
        o.push_str(&format!(
            "| [{}](#{}) | {} | {} |\n",
            e.code,
            e.code.to_ascii_lowercase(),
            severity_word(e.severity),
            e.title
        ));
    }
    for e in &all {
        o.push('\n');
        o.push_str(&render_markdown(e));
    }
    o
}

/// `rulec explain --all --format json`: one object per line, so it streams like the
/// diagnostics themselves.
pub fn json_all() -> String {
    ledger().iter().map(|e| format!("{}\n", render_json(e))).collect()
}

/// `rulec explain --all` in text.
pub fn text_all() -> String {
    ledger().iter().map(|e| format!("{}\n", render_text(e))).collect::<Vec<_>>().join("")
}
