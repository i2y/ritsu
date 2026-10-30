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
//!
//! An example whose names, strings or units are Japanese has an English twin
//! ([`Entry::english`]): the same rule written with English names (`JPY` for `円`), which
//! `explain` shows in English while `--lang ja` shows the Japanese one as it was. The test
//! runs both, and requires both to print the code.

use crate::diag::Severity;
use crate::i18n::Lang;
use crate::json;

/// A reproduction: the smallest rule, and what has to sit next to it as (name, contents).
#[derive(Clone, Copy)]
pub struct Repro {
    pub example: &'static str,
    pub files: &'static [(&'static str, &'static str)],
}

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
    /// The smallest `.rule` that produces it. It is what `explain` shows in Japanese, and in
    /// English too unless [`Entry::english`] is set.
    pub example: &'static str,
    /// Only E109 needs this: with the default budget the example would have to be enormous,
    /// so it is reproduced with a budget this small.
    pub budget: Option<i64>,
    /// What has to sit next to the example for it to reproduce, as (name, contents). Only the
    /// codes about `import proto` need it: their smallest reproduction is two files, and one
    /// of them is not a rule (§15.59). The test writes them beside the example and runs it
    /// there, so a companion cannot rot any more than the example can.
    pub files: &'static [(&'static str, &'static str)],
    /// The same reproduction written with English names, when `example` has Japanese ones:
    /// what `explain` shows in English. The budget is the same for both.
    pub english: Option<Repro>,
    /// Forms of the code the first reproduction does not show, each with a reproduction of its
    /// own: E102's third form, a row past what a derive reaches (§15.189), past what a `define`
    /// reaches, and past what a derive reaches while a table above writes the value the row asks
    /// for (§15.195), and the forms of the other entries `tests/codes.rs` lists.
    pub also: Vec<Also>,
    pub related: &'static [&'static str],
}

/// Another form of a code, reproduced: a heading saying which form it is, and the smallest rule
/// that shows it — with an English twin when the rule has Japanese names, as [`Entry::english`].
pub struct Also {
    pub what: String,
    pub example: &'static str,
    pub english: Option<&'static str>,
    /// What has to sit next to it, as for the entry's own reproduction: the rule a contribution
    /// adds to, a rule an `apply` reads (§15.227, §15.230).
    pub files: &'static [(&'static str, &'static str)],
}

impl Also {
    /// The reproduction `explain` shows in `lang`.
    pub fn repro(&self, lang: Lang) -> Repro {
        match (lang, self.english) {
            (Lang::En, Some(en)) => Repro { example: en, files: self.files },
            _ => Repro { example: self.example, files: self.files },
        }
    }
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
    Entry { code, severity, title, when, fix, example, budget: None, files: &[], english: None, also: Vec::new(), related }
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

    /// Only the `import proto` codes use this (see the field).
    fn with_files(mut self, fs: &'static [(&'static str, &'static str)]) -> Entry {
        self.files = fs;
        self
    }

    /// The same entry with its English twin: the reproduction written with English names,
    /// beside the same files as the Japanese one (see the field).
    fn english(mut self, example: &'static str) -> Entry {
        self.english = Some(Repro { example, files: self.files });
        self
    }

    /// The English twin with files of its own, for a reproduction whose files have Japanese
    /// names or contents too.
    fn english_with_files(mut self, example: &'static str, files: &'static [(&'static str, &'static str)]) -> Entry {
        self.english = Some(Repro { example, files });
        self
    }

    /// Another form of the code, reproduced (see the field).
    fn also(mut self, what: String, example: &'static str, english: &'static str) -> Entry {
        self.also.push(Also { what, example, english: Some(english), files: &[] });
        self
    }

    /// Another form of the code, reproduced with files beside it, written with English names
    /// alone (§15.227).
    fn also_with(mut self, what: String, example: &'static str, files: &'static [(&'static str, &'static str)]) -> Entry {
        self.also.push(Also { what, example, english: None, files });
        self
    }

    /// The reproduction `explain` shows in `lang`: the English twin in English when there is
    /// one, the entry's own otherwise.
    pub fn repro(&self, lang: Lang) -> Repro {
        match (lang, self.english) {
            (Lang::En, Some(en)) => en,
            _ => Repro { example: self.example, files: self.files },
        }
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
const X_E002: &str = "rule t(t) v1\n\ninputs\n  %x(x) : bool\n";
const X_E003: &str = "inputs\n  x(x) : bool\n";
const X_E004: &str = "rule t(t) v1\n\n= 1\n";
const X_E005: &str = "rule t(t) v1\n\nfoo bar\n";
const X_E006: &str = "rule t(t) v1\n\nenum k(k) a(a) | b(b)\n";
const X_E007: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy any\n| x | -> r(r) : bool |\n| - | true           |\n";
const X_E008: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : bool |\n|   | true           |\n";
const X_E009: &str = "rule t(t) v1\n\nenum range(kind) = a(a) | b(b)\n";
const X_E010: &str = "rule t(t) v1\n\ninputs\n  w(w) : mass[g]  range >=0g <=10kg\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| w         | -> r(r) : bool |\n\
                      | 0g..1000g | true           |\n| >1000g    | false          |\n";
const X_E011: &str = "rule t(t) v1\n\ninputs\n  重量 : bool\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| 重量 | -> r(r) : bool |\n| -    | true           |\n";
const X_E011_SEQ: &str = r##"rule 明細の合計(lines_total) v1

elements 明細
  金額(amount) : money[USD]  range >=0USD <=100USD

outputs
  合計(total) : money[USD]  round down(1USD)

sum 支払い(spent) over 明細 of 金額  range >=0USD <=1000USD

define 合計 : money[USD] = 支払い
"##;
const X_E011_SEQ_EN: &str = r##"rule pieces_total v1

elements pièces
  amount : money[USD]  range >=0USD <=100USD

outputs
  total : money[USD]  round down(1USD)

sum spent over pièces of amount  range >=0USD <=1000USD

define total : money[USD] = spent
"##;
const X_E012: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| y | -> r(r) : bool |\n| - | true           |\n";
const X_E013: &str = "rule t(t) v1\n\nimport std/都道府\n";
const X_E013_EN: &str = "rule t(t) v1\n\nimport std/us/state\n";

/// The `.proto` that E032 and E033 are read against. Two values and proto3's zero value,
/// which is "not set" and not a value a table answers for.
const TIER_PROTO: &[(&str, &str)] = &[(
    "tier.proto",
    "syntax = \"proto3\";\n\nenum Tier {\n  TIER_UNSPECIFIED = 0;\n  TIER_ONE = 1;\n  TIER_TWO = 2;\n}\n",
)];

const X_E032: &str = "rule t(t) v1\n\nimport proto \"tier.proto\" Tier -> v\n\
                      enum v(v) = one(one)\n\n\
                      inputs\n  x(x) : v\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x   | -> r(r) : bool |\n| one | true           |\n";

const X_E033: &str = "rule t(t) v1\n\nimport proto \"tier.proto\" Tier -> v\n\
                      enum v(v) = one(one) | two(two)\n\n\
                      inputs\n  x(x) : v\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy first\n| x   | -> r(r) : bool |\n| one | true           |\n| -   | false          |\n";
const X_E014: &str = "rule t(t) v1\n\ninputs\n  p(p) : money[円, incl_tax]  range >=0円 <=1万円\n  \
                      r(r) : rate[step 1%]  range >=0% <=100%\n\n\
                      outputs\n  o(o) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy first\n| r    | -> o(o) : money[円, incl_tax] |\n\
                      | <=5% | 0円                           |\n| -    | p × r                         |\n";
const X_E014_EN: &str = r##"rule t(t) v1

inputs
  p(p) : money[JPY, incl_tax]  range >=0JPY <=10_000JPY
  r(r) : rate[step 1%]  range >=0% <=100%

outputs
  o(o) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy first
| r    | -> o(o) : money[JPY, incl_tax] |
| <=5% | 0JPY                           |
| -    | p * r                          |
"##;

const X_E015: &str = "rule t(t) v1\n\ninputs\n  p(p) : money[円, incl_tax]  range >=0円 <=1万円\n\n\
                      outputs\n  a(a) : money[円, incl_tax]  round down(1円)\n  \
                      b(b) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| p | -> a(a) : money[円, incl_tax] |\n| - | 100円                         |\n\n\
                      result b = p\n";
const X_E015_EN: &str = r##"rule t(t) v1

inputs
  p(p) : money[JPY, incl_tax]  range >=0JPY <=10_000JPY

outputs
  a(a) : money[JPY, incl_tax]  round down(1JPY)
  b(b) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| p | -> a(a) : money[JPY, incl_tax] |
| - | 100JPY                         |

result b = p
"##;
const X_E016: &str = "rule t(t) v1\n\ninputs\n  p(p) : money[円, incl_tax]  range >=0円 <=1万円\n\n\
                      outputs\n  a(a) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| p | -> a(a) : money[円, incl_tax] |\n| - | 100円                         |\n\n\
                      result a = p\nresult a = p + 100円\n";
const X_E016_EN: &str = r##"rule t(t) v1

inputs
  p(p) : money[JPY, incl_tax]  range >=0JPY <=10_000JPY

outputs
  a(a) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| p | -> a(a) : money[JPY, incl_tax] |
| - | 100JPY                         |

result a = p
result a = p + 100JPY
"##;

const X_E017: &str = "rule t(t) v1\n\ninputs\n  a(a) : number  range >=0 <=10\n  \
                      b(b) : number  range >=0 <=10\n\nconstraint a\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| a | -> r(r) : bool |\n| - | true           |\n";
const X_E018: &str = "rule t(t) v1\n\ninputs\n  a(a) : number  range >=0 <=10\n\n\
                      constraint a <= r\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| a | -> r(r) : bool |\n| - | true           |\n";
const X_E019: &str = "rule t(t) v1\n\ninputs\n  a(a) : number  range >=0 <=10\n  \
                      b(b) : number  range >=0 <=10\n\nconstraint a <= b\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| a | b | -> r(r) : bool |\n| - | - | true           |\n\n\
                      examples\n| a | b | -> r |\n| 5 | 1 | true |\n";

/// A rule that walks a sequence: an element decides a verdict, and the fold reduces the
/// column of verdicts. The three examples below bend one thing each.
#[allow(dead_code)]
const FOLD_HEAD: &str = "rule t(t) v1\n\n\
                      enum v(v) = a(a) | b(b)\n\n\
                      elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| k     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\n\
                      fold d over xs\n";
const X_E022: &str = const_str_e022();
/// E022's second form (§15.219): a `max` with no `empty`.
const X_E022_MAX: &str = "rule t(t) v1\n\n\
                      elements xs(xs)\n  k(k) : number  range >=0 <=10\n\n\
                      outputs\n  r(r) : number  round down(1)\n\n\
                      max r over xs of k\n";
const X_E022_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| k      | -> d(d) : v |
| <=5JPY | a           |
| >5JPY  | b           |

fold d over xs
  a -> next
  b -> take_first k
  exhausted -> held
"##;
const X_E023: &str = const_str_e023();
const X_E023_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| k      | -> d(d) : v |
| <=5JPY | a           |
| >5JPY  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0JPY
"##;
const X_E024: &str = const_str_e024();
const X_E024_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| k      | -> d(d) : v |
| <=5JPY | a           |
| >5JPY  | b           |

fold d over xs
  a -> next
  empty -> 0JPY
  exhausted -> held
"##;
const X_E025: &str = const_str_e025();
const X_E025_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| k      | -> d(d) : v |
| <=5JPY | a           |
| >5JPY  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0JPY
  exhausted -> held

examples
| -> r |
| 0JPY |
"##;
const fn const_str_e022() -> &'static str {
    // Every arm and `exhausted`, but no `empty`.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  exhausted -> held\n"
}

const fn const_str_e023() -> &'static str {
    // Every arm and `empty`, but no `exhausted`.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n"
}

const fn const_str_e024() -> &'static str {
    // Both answers, but the verdict `b` has no arm.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\n\
     fold d over xs\n  a -> next\n  empty -> 0円\n  exhausted -> held\n"
}

const fn const_str_e025() -> &'static str {
    // Examples for a rule that walks a sequence, with no column saying which one.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n  exhausted -> held\n\n\
     examples\n| -> r |\n| 0円  |\n"
}

const fn const_str_e026() -> &'static str {
    // A sequence whose column is not a field of an element.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n  exhausted -> held\n\n\
     sequence s(s)\n| m   |\n| 3円 |\n"
}

const fn const_str_e027() -> &'static str {
    // An example naming a sequence nobody wrote.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n  exhausted -> held\n\n\
     examples\n| xs   | -> r |\n| nope | 0円  |\n"
}

const fn const_str_w116() -> &'static str {
    // A sequence written and never named.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\n\
     fold d over xs\n  a -> next\n  b -> take_first k\n  empty -> 0円\n  exhausted -> held\n\n\
     sequence s(s)\n| k   |\n| 3円 |\n"
}

const X_E026: &str = const_str_e026();
const X_E026_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| k      | -> d(d) : v |
| <=5JPY | a           |
| >5JPY  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0JPY
  exhausted -> held

sequence s(s)
| m    |
| 3JPY |
"##;
const X_E027: &str = const_str_e027();
const X_E027_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| k      | -> d(d) : v |
| <=5JPY | a           |
| >5JPY  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0JPY
  exhausted -> held

examples
| xs   | -> r |
| nope | 0JPY |
"##;
const X_E034: &str = r##"rule 重さの判定(weight_check) v1

inputs
  重さ(weight) : number  range >=0 <=9
  重さ(weight) : number  range >=0 <=99

outputs
  重い(heavy) : bool

table 判定(decide)
policy unique
| 重さ | -> 重い |
| <=5  | false   |
| >5   | true    |
"##;
const X_E034_EN: &str = r##"rule weight_check v1

inputs
  weight : number  range >=0 <=9
  weight : number  range >=0 <=99

outputs
  heavy : bool

table decide
policy unique
| weight | -> heavy |
| <=5    | false    |
| >5     | true     |
"##;
const X_E034_VALUE: &str = r##"rule 送料無料(free_shipping) v1

enum 会員区分(member_kind) = 一般(basic) | ゴールド(gold) | 一般(basic)

inputs
  会員(member) : 会員区分

outputs
  無料(free) : bool

table 判定(decide)
policy unique
| 会員     | -> 無料 |
| 一般     | false   |
| ゴールド | true    |
"##;
const X_E034_VALUE_EN: &str = r##"rule free_shipping v1

enum member_kind = basic | gold | basic

inputs
  member : member_kind

outputs
  free : bool

table decide
policy unique
| member | -> free |
| basic  | false   |
| gold   | true    |
"##;
const X_E034_ALIAS: &str = r##"rule 大きさの判定(size_check) v1

inputs
  重さ(weight) : number  range >=0 <=9
  長さ(weight) : number  range >=0 <=9

outputs
  大きい(large) : bool

table 判定(decide)
policy first
| 重さ | 長さ | -> 大きい |
| >5   | -    | true      |
| -    | >5   | true      |
| -    | -    | false     |
"##;
const X_E034_ALIAS_EN: &str = r##"rule size_check v1

inputs
  weight(w) : number  range >=0 <=9
  width(w)  : number  range >=0 <=9

outputs
  large : bool

table decide
policy first
| weight | width | -> large |
| >5     | -     | true     |
| -      | >5    | true     |
| -      | -     | false    |
"##;
const X_E034_COLUMN: &str = r##"rule 重さの判定(weight_check) v1

inputs
  重さ(weight) : number  range >=0 <=9

outputs
  重い(heavy) : bool

table 判定(decide)
policy first
| 重さ | 重さ | -> 重い |
| <=5  | >5   | true    |
| -    | -    | false   |
"##;
const X_E034_COLUMN_EN: &str = r##"rule weight_check v1

inputs
  weight : number  range >=0 <=9

outputs
  heavy : bool

table decide
policy first
| weight | weight | -> heavy |
| <=5    | >5     | true     |
| -      | -      | false    |
"##;
const X_E034_TYPE: &str = r##"rule 段階ごとの料金(tier_fee) v1

enum 会員の段階(tier_level) = 一般(basic) | ゴールド(gold)
enum 地域の段階(tierLevel) = 近い(low) | 遠い(high)

inputs
  会員(member) : 会員の段階
  地域(band)   : 地域の段階

outputs
  料金(fee) : money[USD]  round down(1USD)

table 会員ごとの料金(fee_by_member)
policy unique
| 会員     | 地域 | -> 料金 |
| 一般     | -    | 1USD    |
| ゴールド | 近い | 2USD    |
| ゴールド | 遠い | 3USD    |
"##;
const X_E034_TYPE_EN: &str = r##"rule tier_fee v1

enum tier_level = basic | gold
enum tierLevel = low | high

inputs
  member : tier_level
  band   : tierLevel

outputs
  fee : money[USD]  round down(1USD)

table fee_by_member
policy unique
| member | band | -> fee |
| basic  | -    | 1USD   |
| gold   | low  | 2USD   |
| gold   | high | 3USD   |
"##;
const X_E034_LABEL: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
table 表(t1)\n   | a     | -> x  |\nr1 | true  | true  |\nr1 | false | false |\n";
const X_E034_LABEL_EN: &str = r##"rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table t1(t1)
   | a     | -> x  |
r1 | true  | true  |
r1 | false | false |
"##;
const X_E035: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
table 表(t1)\noverrides 無い表\n| a     | -> x  |\n| true  | true  |\n| false | false |\n";
const X_E035_EN: &str = r##"rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table t1(t1)
overrides no_such_table
| a     | -> x  |
| true  | true  |
| false | false |
"##;
const X_E036: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n  y(y) : bool\n\n\
table 甲(ko)\n| a | -> x |\n| - | true |\n\ntable 乙(otsu)\noverrides 甲\n| a | -> y |\n| - | true |\n";
const X_E036_EN: &str = r##"rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool
  y(y) : bool

table base(base)
| a | -> x |
| - | true |

table special(special)
overrides base
| a | -> y |
| - | true |
"##;
const X_E045: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n  y(y) : bool\n\n\
table 甲(ko)\n| a | -> x | y    |\n| - | true | true |\n\ntable 乙(otsu)\noverrides 甲\n| a    | -> x  |\n| true | false |\n";
const X_E045_EN: &str = r##"rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool
  y(y) : bool

table base(base)
| a | -> x | y    |
| - | true | true |

table special(special)
overrides base
| a    | -> x  |
| true | false |
"##;
const X_W117: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
table 甲(ko)\n   | a     | -> x  |\nr1 | true  | true  |\nr2 | false | false |\n\n\
table 乙(otsu)\noverrides 甲:r1, 甲:r2\n| a    | -> x  |\n| true | false |\n";
const X_W117_EN: &str = r##"rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

table base(base)
   | a     | -> x  |
r1 | true  | true  |
r2 | false | false |

table special(special)
overrides base:r1, base:r2
| a    | -> x  |
| true | false |
"##;
const X_E046: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
clause 例外(exception) -> x\n  then true\n";
const X_E046_EN: &str = r##"rule t(t) v1

inputs
  a(a) : bool

outputs
  x(x) : bool

clause exception(exception) -> x
  then true
"##;
const X_E047: &str = "rule t(t) v1\n\ninputs\n  a(a) : money[円, incl_tax]  range >=0円 <=10000円 incl_tax\n\n\
outputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
const X_E047_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY, incl_tax]  range >=0JPY <=10_000JPY incl_tax

outputs
  x(x) : bool

table t1(t1)
policy unique
| a | -> x |
| - | true |
"##;
const X_E049: &str = "rule t(t) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=5000円\n\n\
outputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a         | -> x  |\n| <=1,000円 | true  |\n| >1,000円  | false |\n";
const X_E049_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY]  range >=0JPY <=5000JPY

outputs
  x(x) : bool

table t1(t1)
policy unique
| a          | -> x  |
| <=1,000JPY | true  |
| >1,000JPY  | false |
"##;
const X_E048: &str = "rule t(t) v1\n\ninputs\n  甲(a) : temperature[℃]  range >=0℃ <=40℃\n  \
乙(b) : temperature[℃]  range >=0℃ <=40℃\n\noutputs\n  x(x) : bool\n\n\
derive 差(gap) : temperature[℃] = 甲 - 乙  range >=-40℃ <=40℃\n\n\
table 表(t1)\npolicy unique\n| 差 | -> x |\n| -  | true |\n";
const X_E048_EN: &str = r##"rule t(t) v1

inputs
  a(a) : temperature[℃]  range >=0℃ <=40℃
  b(b) : temperature[℃]  range >=0℃ <=40℃

outputs
  x(x) : bool

derive gap(gap) : temperature[℃] = a - b  range >=-40℃ <=40℃

table t1(t1)
policy unique
| gap | -> x |
| -   | true |
"##;
/// A copy of one fragment of a law, beside the examples that cite it. Its digest is what the
/// examples pin (or fail to).
const ARTICLE_1: &[(&str, &str)] = &[(
    "sources/law/000AC0000000001@2026-04-01/MainProvision-Article_1.xml",
    "<Article Num=\"1\"><ArticleTitle>第一条</ArticleTitle><Paragraph Num=\"1\"><ParagraphNum/><ParagraphSentence><Sentence>甲は、乙とする。</Sentence></ParagraphSentence></Paragraph></Article>\n",
)];
/// The document of the E116 and W120 examples, and the copy of its table: two files, because
/// the smallest reproduction of "the row and the copy disagree" needs a copy to disagree with
/// (§15.82).
const TARIFF_DOC: &[(&str, &str)] = &[
    ("料金表.md", "# 料金表\n\n| あて先 | 運賃 |\n|---|---|\n| 近畿 | 990円 |\n| 関東 | 880円 |\n"),
    ("料金表.md.fragments/表1.tsv", "あて先\t運賃\n近畿\t990円\n関東\t880円\n"),
];
const TARIFF_DOC_EN: &[(&str, &str)] = &[
    ("tariff.md", "# Tariff\n\n| Destination | Fare |\n|---|---|\n| Kinki | 990JPY |\n| Kanto | 880JPY |\n"),
    ("tariff.md.fragments/table1.tsv", "Destination\tFare\nKinki\t990JPY\nKanto\t880JPY\n"),
];
const X_E116: &str = "rule t(t) v1\n\nsource 料金表 = file \"料金表.md\" sha256:75465b330d123ab8\n  表1 sha256:0a95cedbd7311274\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : money[円]  round down(1円)\n\ntable 表(t1)  @料金表 表1\npolicy unique\n| a     | -> x  |\n| true  | 990円 |\n| false | 890円 |\n";
const X_E116_EN: &str = r##"rule t(t) v1

source tariff = file "tariff.md" sha256:583beb1ff0102e2e
  table1 sha256:07423c36f9e9259f

inputs
  a(a) : bool

outputs
  x(x) : money[JPY]  round down(1JPY)

table t1(t1)  @tariff table1
policy unique
| a     | -> x   |
| true  | 990JPY |
| false | 890JPY |
"##;
const X_W120: &str = "rule t(t) v1\n\nsource 料金表 = file \"料金表.md\" sha256:75465b330d123ab8\n  表1 sha256:0a95cedbd7311274\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : money[円]  round down(1円)\n\ntable 表(t1)  @料金表 表1\npolicy unique\n| a | -> x  |\n| - | 990円 |\n";
const X_W120_EN: &str = r##"rule t(t) v1

source tariff = file "tariff.md" sha256:583beb1ff0102e2e
  table1 sha256:07423c36f9e9259f

inputs
  a(a) : bool

outputs
  x(x) : money[JPY]  round down(1JPY)

table t1(t1)  @tariff table1
policy unique
| a | -> x   |
| - | 990JPY |
"##;
/// The document of the E119 example, and the copy of its table. A second document, because
/// the boundary check needs a table that cuts a number line and the E116 one lists a price
/// per destination (§15.124).
const SIZE_DOC: &[(&str, &str)] = &[
    ("寸法表.md", "# 寸法表\n\n| サイズ | 運賃 |\n|---|---|\n| 60cmまで | 1410円 |\n| 60cmを超え80cm以下 | 1710円 |\n"),
    ("寸法表.md.fragments/表1.tsv", "サイズ\t運賃\n60cmまで\t1410円\n60cmを超え80cm以下\t1710円\n"),
];
const SIZE_DOC_EN: &[(&str, &str)] = &[
    ("sizes.md", "# Sizes\n\n| Size | Fare |\n|---|---|\n| up to 60cm | 1410JPY |\n| over 60cm up to 80cm | 1710JPY |\n"),
    ("sizes.md.fragments/table1.tsv", "Size\tFare\nup to 60cm\t1410JPY\nover 60cm up to 80cm\t1710JPY\n"),
];
const X_E119: &str = "rule t(t) v1\n\nsource 寸法表 = file \"寸法表.md\" sha256:a19333e262c10371\n  表1 sha256:a5c05813ad703b8e\n\ninputs\n  a(a) : length[cm]  range >=1cm <=80cm\n\noutputs\n  x(x) : money[円]  round up(10円)\n\ntable 表(t1)  @寸法表 表1\npolicy unique\n| a             | -> x   |\n| <60cm         | 1410円 |\n| >=60cm <=80cm | 1710円 |\n";
const X_E119_EN: &str = r##"rule t(t) v1

source sizes = file "sizes.md" sha256:8a2a4c38c68dca29
  table1 sha256:34cc5df35a840201

inputs
  a(a) : length[cm]  range >=1cm <=80cm

outputs
  x(x) : money[JPY]  round up(10JPY)

table t1(t1)  @sizes table1
policy unique
| a             | -> x    |
| <60cm         | 1410JPY |
| >=60cm <=80cm | 1710JPY |
"##;
/// The contract of the `shape` examples: one object with one collection in it, which is
/// what a path and a walk both need (§15.125).
const SHAPE_DOC: &[(&str, &str)] = &[("order.json", "{\"$defs\":{\"Order\":{\"type\":\"object\",\"properties\":{\"lines\":{\"type\":\"array\",\"items\":{\"type\":\"object\",\"properties\":{\"category\":{\"type\":\"string\"}}}}}}}}\n")];
const X_E120: &str = "rule t(t) v1\n\nshape order(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  a(a) : bool  from count order.lines\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
const X_E120_EN: &str = r##"rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool  from count order.lines

outputs
  x(x) : bool

table t1(t1)
policy unique
| a | -> x |
| - | true |
"##;
const X_E121: &str = "rule t(t) v1\n\nshape order(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  a(a) : bool  from order.nope\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
const X_E121_EN: &str = r##"rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool  from order.nope

outputs
  x(x) : bool

table t1(t1)
policy unique
| a | -> x |
| - | true |
"##;
/// A path past `order.lines`, which is an array (§15.254).
const X_E121_THROUGH: &str = r##"rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : string  from order.lines.category

outputs
  x(x) : bool

table t1(t1)
policy unique
| a | -> x |
| - | true |
"##;
const X_W122: &str = "rule t(t) v1\n\nshape order(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
const X_W122_EN: &str = r##"rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : bool

outputs
  x(x) : bool

table t1(t1)
policy unique
| a | -> x |
| - | true |
"##;
/// The contract of the E122 and W123 examples: a collection of at most ten, always there. What
/// the two compare is what the contract lets through against what the input takes (§15.132).
const LINES_DOC: &[(&str, &str)] = &[("order.json", "{\"$defs\":{\"Order\":{\"type\":\"object\",\"properties\":{\"lines\":{\"type\":\"array\",\"maxItems\":10,\"items\":{\"type\":\"object\"}}},\"required\":[\"lines\"]}}}\n")];
const X_E122: &str = "rule t(t) v1\n\nshape order(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  a(a) : number  range >=0 <=5  from count order.lines\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
const X_E122_EN: &str = r##"rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : number  range >=0 <=5  from count order.lines

outputs
  x(x) : bool

table t1(t1)
policy unique
| a | -> x |
| - | true |
"##;
/// A `.proto` whose message relates two of its fields, and leaves two others unrelated.
const QUOTE_PROTO: &[(&str, &str)] = &[(
    "quote.proto",
    "syntax = \"proto3\";\npackage shop.v1;\n\nmessage Quote {\n  option (buf.validate.message).cel = {id: \"express_cap\", expression: \"!this.express || this.weight_g <= 5000\"};\n  int64 min_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];\n  int64 max_g = 2 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];\n  int64 weight_g = 3 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];\n  bool express = 4;\n}\n",
)];
const X_E123: &str = "rule t(t) v1\n\nshape 見積(q) = proto \"quote.proto\" shop.v1.Quote\n\ninputs\n  最小(min_g) : mass[g]  range >=1g <=30kg  from 見積.min_g\n  最大(max_g) : mass[g]  range >=1g <=30kg  from 見積.max_g\n\nconstraint 最小 <= 最大\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| 最小 | -> x |\n| -    | true |\n";
const X_E123_EN: &str = r##"rule t(t) v1

shape quote(q) = proto "quote.proto" shop.v1.Quote

inputs
  minimum(min_g) : mass[g]  range >=1g <=30kg  from quote.min_g
  maximum(max_g) : mass[g]  range >=1g <=30kg  from quote.max_g

constraint minimum <= maximum

outputs
  x(x) : bool

table t1(t1)
policy unique
| minimum | -> x |
| -       | true |
"##;
const X_W124: &str = "rule t(t) v1\n\nshape 見積(q) = proto \"quote.proto\" shop.v1.Quote\n\ninputs\n  重さ(weight) : mass[g]  range >=1g <=30kg  from 見積.weight_g\n  急ぎ(express) : bool  from 見積.express\n\noutputs\n  料金(fee) : money[円]  round up(10円)\n\ntable 料金表(fees)\npolicy first\n| 急ぎ  | 重さ | -> 料金 |\n| true  | >5kg | 3000円  |\n| true  | -    | 1500円  |\n| false | -    | 800円   |\n";
const X_W124_EN: &str = r##"rule t(t) v1

shape quote(q) = proto "quote.proto" shop.v1.Quote

inputs
  weight(weight) : mass[g]  range >=1g <=30kg  from quote.weight_g
  express(express) : bool  from quote.express

outputs
  fee(fee) : money[JPY]  round up(10JPY)

table fees(fees)
policy first
| express | weight | -> fee  |
| true    | >5kg   | 3000JPY |
| true    | -      | 1500JPY |
| false   | -      | 800JPY  |
"##;
const X_W123: &str = "rule t(t) v1\n\nshape order(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  a(a) : number  range >=0 <=20  from count order.lines\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a    | -> x  |\n| <=10 | true  |\n| >10  | false |\n";
const X_W123_EN: &str = r##"rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  a(a) : number  range >=0 <=20  from count order.lines

outputs
  x(x) : bool

table t1(t1)
policy unique
| a    | -> x  |
| <=10 | true  |
| >10  | false |
"##;
const X_E037: &str = "rule t(t) v1\n\nsource 法 = law \"000AC0000000001\" asof 2026-04-01\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
table 表(t1)  @法 第1条\n| a | -> x |\n| - | true |\n";
const X_E038: &str = "rule t(t) v1\n\nsource 法 = law \"000AC0000000001\" asof 2026-04-01\n  第1条 sha256:0000000000000000\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
table 表(t1)  @法 第1条\n| a | -> x |\n| - | true |\n";
const X_E039: &str = "rule t(t) v1\n\nsource 法 = law \"000AC0000000001\" asof 2026-04-01\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
table 表(t1)  @法 第2条\n| a | -> x |\n| - | true |\n";
const X_W119: &str = "rule t(t) v1\n\nsource 法 = law \"000AC0000000001\" asof 2026-04-01\n  第1条 sha256:ce31217424a10206\n  第2条 sha256:0000000000000000\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
table 表(t1)  @法 第1条\n| a | -> x |\n| - | true |\n";
/// The rules the `apply` examples apply, beside them: one that passes check, one whose input
/// is an enum, and one with a hole.
const CALLEE: &[(&str, &str)] = &[("呼び先.rule", "rule 呼び先(callee) v1\n\ninputs\n  a(a) : number  range >=1 <=10\n\noutputs\n  x(x) : number  round down(1)\n\ntable 表(t)\npolicy unique\n   | a   | -> x |\n小 | <=5 | 1    |\n大 | >5  | 2    |\n")];
const CALLEE_EN: &[(&str, &str)] = &[("callee.rule", "rule callee(callee) v1\n\ninputs\n  a(a) : number  range >=1 <=10\n\noutputs\n  x(x) : number  round down(1)\n\ntable t(t)\npolicy unique\n      | a   | -> x |\nsmall | <=5 | 1    |\nlarge | >5  | 2    |\n")];
const ENUM_CALLEE: &[(&str, &str)] = &[("区分の呼び先.rule", "rule 区分の呼び先(enum_callee) v1\n\nenum 区分(kind) = 甲(a) | 乙(b)\n\ninputs\n  a(a) : 区分\n\noutputs\n  x(x) : number  round down(1)\n\ntable 表(t)\npolicy unique\n| a  | -> x |\n| 甲 | 1    |\n| 乙 | 2    |\n")];
const ENUM_CALLEE_EN: &[(&str, &str)] = &[("enum_callee.rule", "rule enum_callee(enum_callee) v1\n\nenum band(kind) = low(a) | mid(b)\n\ninputs\n  a(a) : band\n\noutputs\n  x(x) : number  round down(1)\n\ntable t(t)\npolicy unique\n| a   | -> x |\n| low | 1    |\n| mid | 2    |\n")];
const BROKEN_CALLEE: &[(&str, &str)] = &[("壊れた呼び先.rule", "rule 壊れた呼び先(broken) v1\n\ninputs\n  a(a) : number  range >=1 <=10\n\noutputs\n  x(x) : number  round down(1)\n\ntable 表(t)\npolicy unique\n| a   | -> x |\n| <=5 | 1    |\n")];
const BROKEN_CALLEE_EN: &[(&str, &str)] = &[("broken_callee.rule", "rule broken_callee(broken) v1\n\ninputs\n  a(a) : number  range >=1 <=10\n\noutputs\n  x(x) : number  round down(1)\n\ntable t(t)\npolicy unique\n| a   | -> x |\n| <=5 | 1    |\n")];
const X_E040: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=1 <=10\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"呼び先.rule\"\n  a = n\n  x -> y\n";
const X_E040_EN: &str = r##"rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply call(c) = "callee.rule"
  a = n
  x -> y
"##;
const X_E041: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=1 <=10\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"呼び先.rule\" sha256:369102b8f803dc28\n  b = n\n  x -> y\n";
const X_E041_EN: &str = r##"rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply call(c) = "callee.rule" sha256:1c554496c37f709f
  b = n
  x -> y
"##;
const X_E042: &str = "rule t(t) v1\n\nenum 種別(kind) = 甲(a) | 乙(b) | 丙(c)\n\ninputs\n  k(k) : 種別\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"区分の呼び先.rule\" sha256:2e6f2e04c21cdbb3\n  a = k\n  x -> y\n";
const X_E042_EN: &str = r##"rule t(t) v1

enum kind(kind) = low(a) | mid(b) | high(c)

inputs
  k(k) : kind

outputs
  y(y) : number  round down(1)

apply call(c) = "enum_callee.rule" sha256:5c19644247d1b672
  a = k
  x -> y
"##;
const X_E043: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"呼び先.rule\" sha256:369102b8f803dc28\n  a = n\n  x -> y\n";
const X_E043_EN: &str = r##"rule t(t) v1

inputs
  n(n) : number  range >=0 <=10

outputs
  y(y) : number  round down(1)

apply call(c) = "callee.rule" sha256:1c554496c37f709f
  a = n
  x -> y
"##;
/// E043 of an input of an ordered enum held to a range (§15.252), with English names alone, as the
/// other forms with a rule beside them are: an input with no range bound to the callee's, whose
/// `>=silver` row takes platinum too, which the range does not let in.
const X_E043_ENUM: &str = r##"rule t(t) v1

enum tier = basic < silver < gold < platinum

inputs
  k(k) : tier

outputs
  y(y) : number  round down(1)

apply call(c) = "tier_callee.rule" meaning:137f2373042d3154
  campaign = k
  x -> y
"##;
/// A value that may be absent passed to an input of the callee that is not optional (§15.255).
const X_E043_ABSENT: &str = r##"rule t(t) v1

enum tier = basic < silver < gold < platinum

inputs
  m(m) : tier?

outputs
  y(y) : number  round down(1)

apply a(a) = "absent_callee.rule" meaning:38fa5f7ac6342966
  t = m
  x -> y
"##;
/// The callee beside it: a tier that is always there.
const ABSENT_CALLEE: &[(&str, &str)] = &[(
    "absent_callee.rule",
    "rule absent_callee(absent_callee) v1\n\nenum tier = basic < silver < gold < platinum\n\ninputs\n  t(t) : tier\n\noutputs\n  x(x) : number  round down(1)\n\ntable t1(t1)\npolicy unique\n| t        | -> x |\n| basic    | 1    |\n| >=silver | 2    |\n",
)];
const TIER_CALLEE: &[(&str, &str)] = &[(
    "tier_callee.rule",
    "rule tier_callee(tier_callee) v1\n\nenum tier = basic < silver < gold < platinum\n\ninputs\n  campaign(campaign) : tier  range >=basic <=gold\n\noutputs\n  x(x) : number  round down(1)\n\ntable t(t)\npolicy unique\n| campaign | -> x |\n| basic    | 1    |\n| >=silver | 2    |\n",
)];
const X_E044: &str ="rule t(t) v1\n\ninputs\n  n(n) : number  range >=1 <=10\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"壊れた呼び先.rule\" sha256:c4f9eba5b2949205\n  a = n\n  x -> y\n";
const X_E044_EN: &str = r##"rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply call(c) = "broken_callee.rule" sha256:765fd7738ae6bfc1
  a = n
  x -> y
"##;
const X_W118: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=1 <=10\n\noutputs\n  y(y) : number  round down(1)\n\napply 呼(c) = \"呼び先.rule\" sha256:369102b8f803dc28\n  a = n\n  x -> y\n\nclause 特例(special) -> 呼:x\n  when always\n  then 3\n  overrides 呼:表\n";
const X_W118_EN: &str = r##"rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply call(c) = "callee.rule" sha256:1c554496c37f709f
  a = n
  x -> y

clause special(special) -> call:x
  when always
  then 3
  overrides call:t
"##;
const X_W116: &str = const_str_w116();
const X_W116_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| k      | -> d(d) : v |
| <=5JPY | a           |
| >5JPY  | b           |

fold d over xs
  a -> next
  b -> take_first k
  empty -> 0JPY
  exhausted -> held

sequence s(s)
| k    |
| 3JPY |
"##;

const fn const_str_w115() -> &'static str {
    // `b` is in the enum but no row produces it, and the fold still waits for it.
    "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\n\n\
     elements xs(xs)\n  k(k) : money[円, incl_tax]  range >=0円 <=10円\n\n\
     outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
     table j(j)\npolicy unique\n| k | -> d(d) : v |\n| - | a           |\n\n\
     fold d over xs\n  a -> take_first k\n  b -> next\n  empty -> 0円\n  exhausted -> held\n"
}
const X_W115: &str = const_str_w115();
const X_W115_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)

elements xs(xs)
  k(k) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| k | -> d(d) : v |
| - | a           |

fold d over xs
  a -> take_first k
  b -> next
  empty -> 0JPY
  exhausted -> held
"##;

/// E020 (§15.219): a sequence declared, and nothing that walks it.
const X_E020: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\n\
                      elements xs(xs)\n  k(k) : number  range >=0 <=10\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| n | -> r(r) : bool |\n| - | true           |\n";
const X_E021: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\n\
                      elements xs(xs)\n  k(k) : number  range >=0 <=10\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| n | -> r(r) : bool |\n| - | true           |\n\n\
                      fold r\n  empty -> false\n";

const X_E028: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\n\
                      elements xs(xs)\n  b(b) : bool\n\n\
                      outputs\n  r(r) : bool\n\n\
                      count h(h) where b  range >=0 <=10\n\n\
                      table j(j)\npolicy unique\n| n | -> r(r) : bool |\n| - | true           |\n";
const X_E029: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n  ok(ok) : bool\n\n\
                      elements xs(xs)\n  b(b) : bool\n\n\
                      outputs\n  r(r) : bool\n\n\
                      count h(h) over xs where ok  range >=0 <=10\n\n\
                      table j(j)\npolicy unique\n| h | -> r(r) : bool |\n| - | true           |\n";
const X_E030: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\n\
                      elements xs(xs)\n  b(b) : bool\n\n\
                      outputs\n  r(r) : bool\n\n\
                      count h(h) over xs where b\n\n\
                      table j(j)\npolicy unique\n| h | -> r(r) : bool |\n| - | true           |\n";
/// E031 (§15.219): one item reads a value of one element of each of two sequences.
const X_E031: &str = "rule t(t) v1\n\n\
                      elements xs(xs)\n  a(a) : number  range >=0 <=10\n\n\
                      elements ys(ys)\n  b(b) : number  range >=0 <=10\n\n\
                      outputs\n  r(r) : number  round down(1)\n  s(s) : number  round down(1)\n\n\
                      define both(both) : number = a + b\n\n\
                      sum r over xs of both  range >=0 <=1000\n\n\
                      sum s over ys of b  range >=0 <=1000\n";
/// E031's second form (§15.208): an output that is a value of one element.
const X_E031_OUTPUT: &str = "rule 大きな部品(big_parts) v1\n\n\
                      elements 部品(parts)\n  単価(price) : number  range >=0 <=100\n\n\
                      outputs\n  合計(total) : number  round down(1)\n  大きい(big) : bool\n\n\
                      table 大きさ(big_of)\npolicy unique\n| 単価 | -> 大きい : bool |\n| >=50 | true             |\n| <50  | false            |\n\n\
                      sum 合計 over 部品 of 単価  range >=0 <=10000\n";
const X_E031_OUTPUT_EN: &str = "rule big_parts v1\n\n\
                      elements parts\n  price : number  range >=0 <=100\n\n\
                      outputs\n  total : number  round down(1)\n  big   : bool\n\n\
                      table big_of\npolicy unique\n| price | -> big : bool |\n| >=50  | true          |\n| <50   | false         |\n\n\
                      sum total over parts of price  range >=0 <=10000\n";
/// E075 (§15.208): a word that is a value of the column's enum and the name of an input of it.
const X_E075: &str = "rule 段階の引き継ぎ(carry_stage) v1\n\n\
                      enum 段階(stage) = 新規(fresh) | 前回(previous) | 終了(done)\n\n\
                      inputs\n  前回(previous) : 段階\n  閉じる(close) : bool\n\n\
                      outputs\n  次の段階(next_stage) : 段階\n\n\
                      table 決め方(decide)\npolicy unique\n| 閉じる | -> 次の段階 : 段階 |\n| true   | 終了               |\n| false  | 前回               |\n";
const X_E075_EN: &str = "rule carry_stage v1\n\n\
                      enum stage = fresh | previous | done\n\n\
                      inputs\n  previous : stage\n  close    : bool\n\n\
                      outputs\n  next_stage : stage\n\n\
                      table decide\npolicy unique\n| close | -> next_stage : stage |\n| true  | done                  |\n| false | previous              |\n";
/// E076 (§15.274): a rule that implements a contract's service, held to it. The rule of the first
/// form has an input the contract does not; every other form is the rule without it, beside a
/// contract that differs from [`SHIPPING_PROTO`] in one way.
const X_E076: &str = r##"rule t(t) v1
implements proto "shipping.proto" ShippingService

inputs
  subtotal(subtotal) : money[USD]  range >=0USD <=1000USD
  express(express) : bool

outputs
  shipping(shipping) : money[USD]  round down(1USD)

table fee(fee)
policy first
| express | subtotal | -> shipping |
| true    | -        | 20USD       |
| false   | >=50USD  | 0USD        |
| false   | -        | 8USD        |
"##;
const X_E076_BASE: &str = r##"rule t(t) v1
implements proto "shipping.proto" ShippingService

inputs
  subtotal(subtotal) : money[USD]  range >=0USD <=1000USD

outputs
  shipping(shipping) : money[USD]  round down(1USD)

table fee(fee)
policy first
| subtotal | -> shipping |
| >=50USD  | 0USD        |
| -        | 8USD        |
"##;
const X_E076_ENUM: &str = r##"rule t(t) v1
implements proto "shipping.proto" ShippingService

enum tier(tier) = basic(basic) | gold(gold)

inputs
  tier(tier) : tier

outputs
  shipping(shipping) : money[USD]  round down(1USD)

table fee(fee)
policy unique
| tier  | -> shipping |
| gold  | 0USD        |
| basic | 8USD        |
"##;
/// The contract the rule without `express` fits.
const SHIPPING_PROTO: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto3";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}

message QuoteRequest {
  optional int64 subtotal = 2;
}

message QuoteResponse {
  int64 shipping = 1;
  repeated Row trace = 15;
}

message Row {
  string table = 1;
  int32 row = 2;
  string label = 3;
}
"##,
)];
const SHIPPING_EXTRA: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto3";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}

message QuoteRequest {
  optional int64 subtotal = 2;
  optional bool express = 3;
}

message QuoteResponse {
  int64 shipping = 1;
  repeated Row trace = 15;
}

message Row {
  string table = 1;
  int32 row = 2;
  string label = 3;
}
"##,
)];
const SHIPPING_TYPE: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto3";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}

message QuoteRequest {
  optional string subtotal = 2;
}

message QuoteResponse {
  int64 shipping = 1;
  repeated Row trace = 15;
}

message Row {
  string table = 1;
  int32 row = 2;
  string label = 3;
}
"##,
)];
const SHIPPING_PRESENCE: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto3";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}

message QuoteRequest {
  int64 subtotal = 2;
}

message QuoteResponse {
  int64 shipping = 1;
  repeated Row trace = 15;
}

message Row {
  string table = 1;
  int32 row = 2;
  string label = 3;
}
"##,
)];
const SHIPPING_ENUM: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto3";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}

enum Tier {
  TIER_UNSPECIFIED = 0;
  TIER_BASIC = 1;
  TIER_GOLD = 2;
}

message QuoteRequest {
  optional Tier tier = 3;
}

message QuoteResponse {
  int64 shipping = 1;
  repeated Row trace = 15;
}

message Row {
  string table = 1;
  int32 row = 2;
  string label = 3;
}
"##,
)];
const SHIPPING_NO_TRACE: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto3";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}

message QuoteRequest {
  optional int64 subtotal = 2;
}

message QuoteResponse {
  int64 shipping = 1;
}
"##,
)];
const SHIPPING_SIDE_EFFECTS: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto3";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse);
}

message QuoteRequest {
  optional int64 subtotal = 2;
}

message QuoteResponse {
  int64 shipping = 1;
  repeated Row trace = 15;
}

message Row {
  string table = 1;
  int32 row = 2;
  string label = 3;
}
"##,
)];
const SHIPPING_JSON_NAME: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto3";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}

message QuoteRequest {
  optional int64 subtotal = 2 [json_name = "sub"];
}

message QuoteResponse {
  int64 shipping = 1;
  repeated Row trace = 15;
}

message Row {
  string table = 1;
  int32 row = 2;
  string label = 3;
}
"##,
)];
const SHIPPING_PROTO2: &[(&str, &str)] = &[(
    "shipping.proto",
    r##"syntax = "proto2";

package shop.v1;

service ShippingService {
  rpc Quote(QuoteRequest) returns (QuoteResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
}

message QuoteRequest {
  optional int64 subtotal = 2;
}

message QuoteResponse {
  optional int64 shipping = 1;
  repeated Row trace = 15;
}

message Row {
  optional string table = 1;
  optional int32 row = 2;
  optional string label = 3;
}
"##,
)];
/// W128 (§15.209): a row that asks a computed column for a value no input gives, which nothing
/// shows.
const X_W128: &str = "rule 平方の区分(square_band) v1\n\n\
                      inputs\n  辺(side) : number  range >=0 <=10\n\n\
                      outputs\n  区分(band) : number  round down(1)\n\n\
                      define 面積 : number = 辺 * 辺\n\n\
                      table 区分表(band_of)\npolicy first\n| 面積 | -> 区分 : number |\n| 2    | 1                |\n| -    | 2                |\n";
const X_W128_EN: &str = "rule square_band v1\n\n\
                      inputs\n  side : number  range >=0 <=10\n\n\
                      outputs\n  band : number  round down(1)\n\n\
                      define area : number = side * side\n\n\
                      table band_of\npolicy first\n| area | -> band : number |\n| 2    | 1                |\n| -    | 2                |\n";
const X_E101: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\n\n\
                      inputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : bool |\n\
                      | a | true           |\n| b | false          |\n";
const X_E102: &str = "rule t(t) v1\n\ninputs\n  w(w) : mass[g]  range >=0g <=10kg\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy first\n| w       | -> r(r) : bool |\n\
                      | -       | true           |\n| <=1000g | false          |\n";
// E102's third form (§15.189): `超過` comes to -99円 up to 100円 over the inputs' ranges, so the
// last row, wider in the derive's own `range`, is reached by no input.
const X_E102_REACH: &str = r##"rule t(t) v1

inputs
  額(a)   : money[円, incl_tax]  range >=1円 <=100円
  上限(l) : money[円, incl_tax]  range >=0円 <=100円

outputs
  r(r) : bool

derive 超過(x) : money[円, incl_tax] = 額 - 上限  range >=-1000円 <=1000円

table j(j)
policy unique
| 超過         | -> r(r) : bool |
| <=0円        | false          |
| >0円 <=100円 | true           |
| >100円       | true           |
"##;
const X_E102_REACH_EN: &str = r##"rule t(t) v1

inputs
  amount(a) : money[JPY, incl_tax]  range >=1JPY <=100JPY
  limit(l)  : money[JPY, incl_tax]  range >=0JPY <=100JPY

outputs
  r(r) : bool

derive excess(x) : money[JPY, incl_tax] = amount - limit  range >=-1000JPY <=1000JPY

table j(j)
policy unique
| excess         | -> r(r) : bool |
| <=0JPY         | false          |
| >0JPY <=100JPY | true           |
| >100JPY        | true           |
"##;
// E102's third form on a `define` (§15.195): `割合` comes to 0% up to 100% over the input's range, so
// the last row asks for a share no input makes.
const X_E102_DEFINE: &str = r##"rule t(t) v1

inputs
  点(p) : number  range >=0 <=200

outputs
  r(r) : bool

define 割合(s) : rate = 点 / 200

table j(j)
policy unique
| 割合         | -> r(r) : bool |
| <50%         | false          |
| >=50% <=100% | true           |
| >100%        | true           |
"##;
const X_E102_DEFINE_EN: &str = r##"rule t(t) v1

inputs
  score(p) : number  range >=0 <=200

outputs
  r(r) : bool

define share(s) : rate = score / 200

table j(j)
policy unique
| share        | -> r(r) : bool |
| <50%         | false          |
| >=50% <=100% | true           |
| >100%        | true           |
"##;
// E102's third form on a boolean `define` (§15.196): every day `作成日` takes is before the end of
// the period, so `期間内` is always true and the second row asks for a value no input makes.
const X_E102_TRUTH: &str = r##"rule t(t) v1

inputs
  作成日(d) : date  range >=2024-04-01 <=2026-12-31

outputs
  r(r) : bool

define 期間内(p) : bool = 作成日 <= 2027-03-31

table j(j)
policy unique
| 期間内 | -> r(r) : bool |
| true   | true           |
| false  | false          |
"##;
const X_E102_TRUTH_EN: &str = r##"rule t(t) v1

inputs
  made(d) : date  range >=2024-04-01 <=2026-12-31

outputs
  r(r) : bool

define in_period(p) : bool = made <= 2027-03-31

table j(j)
policy unique
| in_period | -> r(r) : bool |
| true      | true           |
| false     | false          |
"##;
// E102's third form across a table above (§15.195): `区分` is 少額 only for an amount up to 50円, and
// then `超過` is 50円 at most, so the second row of `j` is reached by no input.
const X_E102_ABOVE: &str = r##"rule t(t) v1

enum 大きさ(z) = 少額(small) | 多額(large)

inputs
  額(a)   : money[円, incl_tax]  range >=1円 <=100円
  上限(l) : money[円, incl_tax]  range >=0円 <=100円

outputs
  r(r) : bool

derive 超過(x) : money[円, incl_tax] = 額 - 上限  range >=-100円 <=100円

table k(k)
policy unique
| 額     | -> 区分(c) : 大きさ |
| <=50円 | 少額                |
| >50円  | 多額                |

table j(j)
policy unique
| 区分 | 超過   | -> r(r) : bool |
| 少額 | <=50円 | false          |
| 少額 | >50円  | true           |
| 多額 | -      | true           |
"##;
const X_E102_ABOVE_EN: &str = r##"rule t(t) v1

enum size(z) = small(small) | large(large)

inputs
  amount(a) : money[JPY, incl_tax]  range >=1JPY <=100JPY
  limit(l)  : money[JPY, incl_tax]  range >=0JPY <=100JPY

outputs
  r(r) : bool

derive excess(x) : money[JPY, incl_tax] = amount - limit  range >=-100JPY <=100JPY

table k(k)
policy unique
| amount  | -> band(c) : size |
| <=50JPY | small             |
| >50JPY  | large             |

table j(j)
policy unique
| band  | excess  | -> r(r) : bool |
| small | <=50JPY | false          |
| small | >50JPY  | true           |
| large | -       | true           |
"##;
const X_E103: &str = "rule t(t) v1\n\ninputs\n  w(w) : mass[g]  range >=0g <=10kg\n  \
                      p(p) : money[円, incl_tax]  range >=0円 <=1万円\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| w | -> r(r) : money[円, incl_tax] |\n| - | 100円                         |\n\n\
                      result r = p + w\n";
const X_E103_EN: &str = r##"rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg
  p(p) : money[JPY, incl_tax]  range >=0JPY <=10_000JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| w | -> r(r) : money[JPY, incl_tax] |
| - | 100JPY                         |

result r = p + w
"##;
const X_E103_HEADING: &str = r##"rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg

outputs
  r(r) : money[円, excl_tax]  round down(1円)

table j(j)
policy unique
| w | -> r(r) : money[円, incl_tax] |
| - | 100円                         |
"##;
const X_E103_HEADING_EN: &str = r##"rule t(t) v1

inputs
  w(w) : mass[g]  range >=0g <=10kg

outputs
  r(r) : money[JPY, excl_tax]  round down(1JPY)

table j(j)
policy unique
| w | -> r(r) : money[JPY, incl_tax] |
| - | 100JPY                         |
"##;
const X_E103_NONE: &str = r##"rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[円, incl_tax]  round down(1円)

table j(j)
policy unique
| x     | -> r(r) : money[円, incl_tax] |
| true  | 100円                         |
| false | none                          |
"##;
const X_E103_NONE_EN: &str = r##"rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| x     | -> r(r) : money[JPY, incl_tax] |
| true  | 100JPY                         |
| false | none                           |
"##;
const X_E104: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]\n\n\
                      table j(j)\npolicy unique\n| x     | -> r(r) : money[円, incl_tax] |\n\
                      | true  | 100円                         |\n| false | 200円                         |\n";
const X_E104_EN: &str = r##"rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[JPY, incl_tax]

table j(j)
policy unique
| x     | -> r(r) : money[JPY, incl_tax] |
| true  | 100JPY                         |
| false | 200JPY                         |
"##;
const X_E105: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\n\
                      inputs\n  x(x) : k\n  y(y) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| x | y     | -> r(r) : money[円, incl_tax] |\n\
                      | a | -     | 100円                         |\n| - | true  | 200円                         |\n| b | false | 300円                         |\n";
const X_E105_EN: &str = r##"rule t(t) v1

enum k(k) = a(a) | b(b)

inputs
  x(x) : k
  y(y) : bool

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| x | y     | -> r(r) : money[JPY, incl_tax] |
| a | -     | 100JPY                         |
| - | true  | 200JPY                         |
| b | false | 300JPY                         |
"##;
const X_E106: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round up(10円)\n\n\
                      table j(j)\npolicy unique\n| x     | -> r(r) : money[円, incl_tax] |\n\
                      | true  | 1451円                        |\n| false | 1000円                        |\n";
const X_E106_EN: &str = r##"rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[JPY, incl_tax]  round up(10JPY)

table j(j)
policy unique
| x     | -> r(r) : money[JPY, incl_tax] |
| true  | 1451JPY                        |
| false | 1000JPY                        |
"##;
const X_E107: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| x     | -> r(r) : money[円, incl_tax] |\n\
                      | true  | 100円                         |\n| false | 200円                         |\n\n\
                      examples\n| x    | -> r  |\n| true | 200円 |\n";
const X_E107_EN: &str = r##"rule t(t) v1

inputs
  x(x) : bool

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| x     | -> r(r) : money[JPY, incl_tax] |
| true  | 100JPY                         |
| false | 200JPY                         |

examples
| x    | -> r   |
| true | 200JPY |
"##;
const X_E108: &str = "rule t(t) v1\n\n\
                      inputs\n  p(p) : money[円, incl_tax]  range >=0円 <=100000000000000000円\n  \
                      q(q) : rate[step 1%]  range >=0% <=100%\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      define off(off) : money[円, incl_tax] = p × q\n\n\
                      table j(j)\npolicy unique\n| off | -> r(r) : money[円, incl_tax] |\n| -   | 0円                           |\n";
const X_E108_EN: &str = r##"rule t(t) v1

inputs
  p(p) : money[JPY, incl_tax]  range >=0JPY <=100_000_000_000_000_000JPY
  q(q) : rate[step 1%]  range >=0% <=100%

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

define off(off) : money[JPY, incl_tax] = p * q

table j(j)
policy unique
| off | -> r(r) : money[JPY, incl_tax] |
| -   | 0JPY                           |
"##;
const X_E109: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\n\n\
                      inputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x | -> r(r) : bool |\n\
                      | a | true           |\n| b | false          |\n| c | true           |\n";
const X_E110: &str = "rule t(t) v1\n\ninputs\n  s(s) : string?\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| s               | -> r(r) : bool |\n| starts_with \"a\" | true           |\n";
const X_E111: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\n\
                      outputs\n  ok(ok) : bool\n  fee(fee) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy unique\n| x     | -> ok(ok) : bool | fee(fee) : money[円, incl_tax] |\n\
                      | true  | true             | 100円                          |\n| false | false            | 0円                            |\n\n\
                      examples\n| x    | -> ok |\n| true | true  |\n";
const X_E111_EN: &str = r##"rule t(t) v1

inputs
  x(x) : bool

outputs
  ok(ok) : bool
  fee(fee) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy unique
| x     | -> ok(ok) : bool | fee(fee) : money[JPY, incl_tax] |
| true  | true             | 100JPY                          |
| false | false            | 0JPY                            |

examples
| x    | -> ok |
| true | true  |
"##;
const X_E112: &str = "rule t(t) v1\n\n\
                      inputs\n  a(a) : money[円, incl_tax]  range >=0円 <=100万円\n  \
                      b(b) : money[円, incl_tax]  range >=0円 <=100万円\n\n\
                      outputs\n  r(r) : bool\n\n\
                      derive gap(gap) : money[円, incl_tax] = a - b  range >=0円 <=100万円\n\n\
                      table j(j)\npolicy unique\n| gap   | -> r(r) : bool |\n\
                      | <=0円 | false          |\n| >0円  | true           |\n";
const X_E112_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  b(b) : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY

outputs
  r(r) : bool

derive gap(gap) : money[JPY, incl_tax] = a - b  range >=0JPY <=1_000_000JPY

table j(j)
policy unique
| gap    | -> r(r) : bool |
| <=0JPY | false          |
| >0JPY  | true           |
"##;
const X_E113: &str = "rule t(t) v1\n\n\
                      inputs\n  a(a) : money[円, incl_tax]  range >=0円 <=100万円\n  \
                      b(b) : money[円, incl_tax]  range >=0円 <=100万円\n\n\
                      outputs\n  r(r) : bool\n\n\
                      define bigger(bigger) : bool = a >= b\n\n\
                      table j(j)\npolicy unique\n| bigger | -> r(r) : bool |\n\
                      | true   | true           |\n| false  | false          |\n";
const X_E113_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  b(b) : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY

outputs
  r(r) : bool

define bigger(bigger) : bool = a >= b

table j(j)
policy unique
| bigger | -> r(r) : bool |
| true   | true           |
| false  | false          |
"##;
const X_E114: &str = "rule t(t) v1\n\ninputs\n  r(r) : rate[step 1%]  range >=0% <=100%\n\n\
                      outputs\n  o(o) : bool\n\n\
                      table j(j)\npolicy first\n| r      | -> o(o) : bool |\n\
                      | <=0.5% | true           |\n| -      | false          |\n";
const X_E115: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=100\n  \
                      d(d) : number  range >=1 <=100\n\n\
                      outputs\n  o(o) : bool\n\n\
                      define r(r) : number = n \u{00f7} d\n\n\
                      table j(j)\npolicy first\n| r | -> o(o) : bool |\n| - | true           |\n";

// E132 (§15.213): the average price of an order, divided by a count that can be zero.
const X_E132: &str = "rule 平均単価(average_price) v1\n\n\
                      inputs\n  合計(total) : money[円]  range >=0円 <=10000円\n  件数(units) : number  range >=0 <=100\n\n\
                      outputs\n  平均(average) : money[円]  round half_up(1円)\n\n\
                      define 単価(per_unit) : money[円] = half_up(合計 / 件数, 1円)\n\n\
                      result 平均 = 単価\n";
const X_E132_EN: &str = "rule average_price v1\n\n\
                      inputs\n  total : money[JPY]  range >=0JPY <=10000JPY\n  units : number  range >=0 <=100\n\n\
                      outputs\n  average : money[JPY]  round half_up(1JPY)\n\n\
                      define per_unit : money[JPY] = half_up(total / units, 1JPY)\n\n\
                      result average = per_unit\n";
// E139 (§15.215): a square root of a difference that can go below zero.
const X_E139: &str = "rule 平方根(root_of_gap) v1\n\n\
                      inputs\n  差(gap) : number  range >=-10 <=100\n\n\
                      outputs\n  根(root) : number  round down(1)\n\n\
                      define 整数の根(whole_root) : number = isqrt(差)\n\n\
                      result 根 = 整数の根\n";
const X_E139_EN: &str = "rule root_of_gap v1\n\n\
                      inputs\n  gap : number  range >=-10 <=100\n\n\
                      outputs\n  root : number  round down(1)\n\n\
                      define whole_root : number = isqrt(gap)\n\n\
                      result root = whole_root\n";
// E140 (§15.268): a range written on an output's line that leaves out what the output comes to,
// and one that cannot be checked, the output reading an input with no range.
const X_E140: &str = r##"rule 手数料(fee) v1

inputs
  金額(amount) : money[円]  range >=0円 <=10_000円

outputs
  手数料(fee) : money[円]  round down(1円)  range >=0円 <=500円

result 手数料 = 金額 × 10%
"##;
const X_E140_EN: &str = r##"rule fee v1

inputs
  amount : money[USDc]  range >=0USDc <=10_000USDc

outputs
  fee : money[USDc]  round down(1USDc)  range >=0USDc <=500USDc

result fee = amount × 10%
"##;
const X_E140_UNKNOWN: &str = r##"rule 手数料(fee) v1

inputs
  金額(amount) : money[円]

outputs
  手数料(fee) : money[円]  round down(1円)  range >=0円 <=500円

result 手数料 = 金額 × 10%
"##;
const X_E140_UNKNOWN_EN: &str = r##"rule fee v1

inputs
  amount : money[USDc]

outputs
  fee : money[USDc]  round down(1USDc)  range >=0USDc <=500USDc

result fee = amount × 10%
"##;

const X_E117: &str = "rule t(t) v1\n\ninputs\n  \u{5024}\u{5f15}\u{304d}(off) : money[\u{5186}]  range >=0\u{5186} <=1000\u{5186}\n  \
                      \u{3053}\u{3053}\u{307e}\u{3067}(upto) : money[\u{5186}]  range >=0\u{5186} <=1000\u{5186}\n  \
                      \u{5408}\u{8a08}(base) : money[\u{5186}]  range >=1\u{5186} <=1000\u{5186}\n\n\
                      outputs\n  o(o) : money[\u{5186}]  round down(1\u{5186})\n\n\
                      derive \u{914d}\u{5206}(share) : money[\u{5186}] = allocate(\u{5024}\u{5f15}\u{304d}, \u{3053}\u{3053}\u{307e}\u{3067}, \u{5408}\u{8a08})  range >=0\u{5186} <=1000\u{5186}\n\n\
                      result o = \u{914d}\u{5206}\n";
const X_E117_EN: &str = r##"rule t(t) v1

inputs
  off(off) : money[JPY]  range >=0JPY <=1000JPY
  upto(upto) : money[JPY]  range >=0JPY <=1000JPY
  base(base) : money[JPY]  range >=1JPY <=1000JPY

outputs
  o(o) : money[JPY]  round down(1JPY)

derive share(share) : money[JPY] = allocate(off, upto, base)  range >=0JPY <=1000JPY

result o = share
"##;

const X_E118: &str = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=100\n  \
                      d(d) : number  range >=1 <=100\n\n\
                      outputs\n  o(o) : number  round down(1)\n\n\
                      define r(r) : number = min(n, d, n)\n\n\
                      result o = r\n";

const X_W121: &str = "rule t(t) v1\n\ninputs\n  \u{7a2e}\u{5225}(type) : number  range >=0 <=10\n\n\
                      outputs\n  o(o) : number  round down(1)\n\n\
                      table j(j)\npolicy first\n| \u{7a2e}\u{5225} | -> o(o) : number |\n\
                      | >=5  | 1                |\n| -    | 0                |\n";
const X_W121_EN: &str = r##"rule t(t) v1

inputs
  kind(type) : number  range >=0 <=10

outputs
  o(o) : number  round down(1)

table j(j)
policy first
| kind | -> o(o) : number |
| >=5  | 1                |
| -    | 0                |
"##;

const X_W121_CAMEL: &str = r##"rule 定額の送料(flat_fee) v1

inputs
  数量(qty) : number  range >=0 <=10

outputs
  送料(fee) : money[USD]  round down(1USD)

define 基本料金(base_fee) : money[USD] = 3USD
define 追加料金(baseFee) : money[USD] = 2USD

table 数量ごとの送料(fee_by_qty)
policy unique
| 数量 | -> 送料  |
| <=5  | 基本料金 |
| >5   | 追加料金 |
"##;
const X_W121_CAMEL_EN: &str = r##"rule flat_fee v1

inputs
  qty : number  range >=0 <=10

outputs
  fee : money[USD]  round down(1USD)

define base_fee : money[USD] = 3USD
define baseFee : money[USD] = 2USD

table fee_by_qty
policy unique
| qty | -> fee   |
| <=5 | base_fee |
| >5  | baseFee  |
"##;
const X_W121_CASE: &str = r##"rule 少額の送料(small_fee) v1

inputs
  数量(qty) : number  range >=0 <=10

outputs
  送料(fee) : money[USD]  round down(1USD)

define 基本料金(afee) : money[USD] = 3USD
define 追加料金(aFee) : money[USD] = 2USD

table 数量ごとの送料(fee_by_qty)
policy unique
| 数量 | -> 送料  |
| <=5  | 基本料金 |
| >5   | 追加料金 |
"##;
const X_W121_CASE_EN: &str = r##"rule small_fee v1

inputs
  qty : number  range >=0 <=10

outputs
  fee : money[USD]  round down(1USD)

define afee : money[USD] = 3USD
define aFee : money[USD] = 2USD

table fee_by_qty
policy unique
| qty | -> fee |
| <=5 | afee   |
| >5  | aFee   |
"##;
const X_W121_LONG: &str = r##"rule 春の送料(spring_fee) v1

inputs
  数量(qty) : number  range >=0 <=10

outputs
  送料(fee) : money[USD]  round down(1USD)

define 春のキャンペーンが始まる前に受けた少ない注文の送料その一 : money[USD] = 3USD
define 春のキャンペーンが始まる前に受けた少ない注文の送料その二 : money[USD] = 2USD

table 数量ごとの送料(fee_by_qty)
policy unique
| 数量 | -> 送料                                                  |
| <=5  | 春のキャンペーンが始まる前に受けた少ない注文の送料その一 |
| >5   | 春のキャンペーンが始まる前に受けた少ない注文の送料その二 |
"##;
const X_W121_LONG_EN: &str = r##"rule spring_fee v1

inputs
  qty : number  range >=0 <=10

outputs
  fee : money[USD]  round down(1USD)

define the_fee_for_a_small_order_placed_before_the_spring_campaign_of_2026_began : money[USD] = 3USD
define the_fee_for_a_small_order_placed_before_the_spring_campaign_of_2026_ended : money[USD] = 2USD

table fee_by_qty
policy unique
| qty | -> fee                                                                    |
| <=5 | the_fee_for_a_small_order_placed_before_the_spring_campaign_of_2026_began |
| >5  | the_fee_for_a_small_order_placed_before_the_spring_campaign_of_2026_ended |
"##;
const X_W121_RUBY: &str = r##"rule 基本の送料(base_fee) v1

inputs
  数量(qty) : number  range >=0 <=10

outputs
  送料(fee) : money[USD]  round down(1USD)

define 基本料金(Base) : money[USD] = 3USD

table 数量ごとの送料(fee_by_qty)
policy unique
| 数量 | -> 送料  |
| <=5  | 基本料金 |
| >5   | 2USD     |
"##;
const X_W121_RUBY_EN: &str = r##"rule base_fee v1

inputs
  qty : number  range >=0 <=10

outputs
  fee : money[USD]  round down(1USD)

define Base : money[USD] = 3USD

table fee_by_qty
policy unique
| qty | -> fee |
| <=5 | Base   |
| >5  | 2USD   |
"##;
const X_W121_OWN: &str = r##"rule 送料と記録(fee_and_trace) v1

inputs
  数量(qty) : number  range >=0 <=10

outputs
  送料(fee)   : money[USD]  round down(1USD)
  記録(trace) : number  round down(1)

table 数量ごとの送料(fee_by_qty)
policy unique
| 数量 | -> 送料 | 記録 |
| <=5  | 3USD    | 1    |
| >5   | 2USD    | 2    |
"##;
const X_W121_OWN_EN: &str = r##"rule fee_and_trace v1

inputs
  qty : number  range >=0 <=10

outputs
  fee   : money[USD]  round down(1USD)
  trace : number  round down(1)

table fee_by_qty
policy unique
| qty | -> fee | trace |
| <=5 | 3USD   | 1     |
| >5  | 2USD   | 2     |
"##;
const X_W121_BLANK: &str = r##"rule 下線の手数料(underscore_fee) v1

inputs
  数量(qty) : number  range >=0 <=10

outputs
  手数料(fee) : money[USD]  round down(1USD)

define _ : money[USD] = 3USD

table 数量ごとの手数料(fee_by_qty)
policy unique
| 数量 | -> 手数料 |
| <=5  | _         |
| >5   | 2USD      |
"##;
const X_W121_BLANK_EN: &str = r##"rule underscore_fee v1

inputs
  qty : number  range >=0 <=10

outputs
  fee : money[USD]  round down(1USD)

define _ : money[USD] = 3USD

table fee_by_qty
policy unique
| qty | -> fee |
| <=5 | _      |
| >5  | 2USD   |
"##;

const X_W105: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\n\
                      inputs\n  x(x) : k\n  y(y) : bool\n\n\
                      outputs\n  r(r) : money[円, incl_tax]  round down(1円)\n\n\
                      table j(j)\npolicy first\n| x | y    | -> r(r) : money[円, incl_tax] |\n\
                      | a | -    | 100円                         |\n| - | true | 200円                         |\n| - | -    | 300円                         |\n";
const X_W105_EN: &str = r##"rule t(t) v1

enum k(k) = a(a) | b(b)

inputs
  x(x) : k
  y(y) : bool

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)

table j(j)
policy first
| x | y    | -> r(r) : money[JPY, incl_tax] |
| a | -    | 100JPY                         |
| - | true | 200JPY                         |
| - | -    | 300JPY                         |
"##;
const X_W110: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b)\n\n\
                      inputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy first\n| x | -> r(r) : bool |\n\
                      | a | true           |\n| b | false          |\n";
const X_W111: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n  w(w) : mass[g]  range >=0g <=10kg\n\n\
                      outputs\n  r(r) : bool\n\n\
                      table j(j)\npolicy unique\n| x     | -> r(r) : bool |\n\
                      | true  | true           |\n| false | false          |\n";
const X_W114: &str = "rule t(t) v1\n\ninputs\n  a(a) : money[円]  range >=0円 <=10万円\n\noutputs\n  r(r) : bool\n\nderive 倍(d) : money[円] = a + a  range >=0円 <=20万円\n\ndefine 上(up) : bool = 倍 >= 5円\ndefine 下(dn) : bool = 倍 <= 5円\n\ntable j(j)\npolicy unique\n| 上    | 下    | -> r(r) : bool |\n| true  | -     | true           |\n| -     | true  | false          |\n| false | false | false          |\n";
const X_W114_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY]  range >=0JPY <=100_000JPY

outputs
  r(r) : bool

derive twice(d) : money[JPY] = a + a  range >=0JPY <=200_000JPY

define above(up) : bool = twice >= 5JPY
define below(dn) : bool = twice <= 5JPY

table j(j)
policy unique
| above | below | -> r(r) : bool |
| true  | -     | true           |
| -     | true  | false          |
| false | false | false          |
"##;

// A state machine (§15.148): three states, two events, one table of transitions.
const X_E050: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | a             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n\nmachine k(k) over m\n  carry   st -> nx\n";
const X_E051: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | a             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n\nmachine k(k) over m\n  carry   zz -> nx\n  initial a\n";
const X_E052: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | a             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial a\n  final   d\n";
const X_E053: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | a             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial a\n  once    nx c\n";
const X_E054: &str = "rule t(t) v1\n\nenum v(v) = a(a) | b(b)\nenum s(s) = p(p) | q(q)\n\ninputs\n  st(st) : s\n\nelements xs(xs)\n  w(w) : money[円, incl_tax]  range >=0円 <=10円\n\noutputs\n  r(r) : money[円, incl_tax]  round down(1円)\n  nx(nx) : s\n\ntable j(j)\npolicy unique\n| w     | -> d(d) : v |\n| <=5円 | a           |\n| >5円  | b           |\n\ntable m(m)\npolicy unique\n| st | -> nx(nx) : s |\n| p  | q             |\n| q  | q             |\n\nfold d over xs\n  a -> next\n  b -> take_first w\n  empty -> 0円\n  exhausted -> held\n\nmachine k(k) over m\n  carry   st -> nx\n  initial p\n";
const X_E054_EN: &str = r##"rule t(t) v1

enum v(v) = a(a) | b(b)
enum s(s) = p(p) | q(q)

inputs
  st(st) : s

elements xs(xs)
  w(w) : money[JPY, incl_tax]  range >=0JPY <=10JPY

outputs
  r(r) : money[JPY, incl_tax]  round down(1JPY)
  nx(nx) : s

table j(j)
policy unique
| w      | -> d(d) : v |
| <=5JPY | a           |
| >5JPY  | b           |

table m(m)
policy unique
| st | -> nx(nx) : s |
| p  | q             |
| q  | q             |

fold d over xs
  a -> next
  b -> take_first w
  empty -> 0JPY
  exhausted -> held

machine k(k) over m
  carry   st -> nx
  initial p
"##;
const X_E055: &str = "rule t(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x     | -> r(r) : bool |\n| true  | false          |\n| false | true           |\n\nscenario s(s)\n| x    | -> r  |\n| true | false |\n";
/// E056: a `held` line naming an output.
/// The rule the reproductions of `prerequisites` (§15.222) are written over: three values, a table
/// that reads them, and the block under it.
const X_E071: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\n\ninputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x | -> r(r) : bool |\n| - | true           |\n\nprerequisites p(p) over k\n  start a\n  b     requires a\n";
const X_E136: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\n\ninputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x | -> r(r) : bool |\n| - | true           |\n\nprerequisites p(p) over k\n  start a\n  b     requires c\n  c     requires b\n";
const X_E137: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\n\ninputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x | -> r(r) : bool |\n| - | true           |\n\nprerequisites p(p) over k\n  start a\n  b     requires c\n  c     requires nothing \"retired\"\n";
const X_E138: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\nenum s(s) = one(one) < two(two)\n\ninputs\n  x(x) : k\n\noutputs\n  r(r) : s\n\ntable j(j)\npolicy unique\n| x    | -> r(r) : s |\n| a, c | one         |\n| b    | two         |\n\nprerequisites p(p) over k\n  start a\n  b     requires a\n  c     requires b\n  ordered by j\n";
const X_W130: &str = "rule t(t) v1\n\nenum k(k) = a(a) | b(b) | c(c)\n\ninputs\n  x(x) : k\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| x | -> r(r) : bool |\n| - | true           |\n\nprerequisites p(p) over k\n  start a\n  b     requires a\n  c     requires b, a\n";
const X_E072: &str = "rule t(t) v1\n\nenum s(s) = run(run) | done(done)\n\ninputs\n  st(st) : s\n  n(n)   : number  range >=0 <=3\n\noutputs\n  nx(nx) : s\n  nn(nn) : number  round down(1)\n\ndefine more(more) : number = n + 1\n\ntable m(m)\npolicy first\n| st  | n  | -> nx(nx) : s | nn(nn) : number |\n| run | <3 | run           | more            |\n| -   | -  | done          | n               |\n\nmachine k(k) over m\n  carry   st -> nx\n  carry   n -> nn\n  initial run\n";
const X_E134: &str = "rule t(t) v1\n\nenum s(s) = run(run) | done(done)\n\ninputs\n  st(st)       : s\n  again(again) : bool\n  n(n)         : number  range >=0 <=3\n\noutputs\n  nx(nx) : s\n  nn(nn) : number  round down(1)\n\ndefine more(more) : number = n + 1\n\ntable m(m)\npolicy first\n| st   | again | n   | -> nx(nx) : s | nn(nn) : number |\n| run  | true  | <=2 | run           | more            |\n| run  | true  | -   | done          | more            |\n| run  | -     | -   | done          | n               |\n| done | -     | -   | done          | n               |\n\nmachine k(k) over m\n  carry   st -> nx\n  carry   n -> nn\n  initial run\n  initial n 0\n";
const X_E135: &str = "rule t(t) v1\n\nenum s(s) = run(run) | done(done)\n\ninputs\n  st(st) : s\n  n(n)   : number  range >=0 <=3\n\noutputs\n  nx(nx) : s\n  nn(nn) : number  round down(1)\n\ndefine more(more) : number = n + 1\n\ntable m(m)\npolicy first\n| st  | n  | -> nx(nx) : s | nn(nn) : number |\n| run | <3 | run           | more            |\n| -   | -  | done          | n               |\n\nmachine k(k) over m\n  carry   st -> nx\n  carry   n -> nn\n  initial run\n  initial n 0\n  always  n <=2 when run\n";
const X_E056: &str = "rule t(t) v1\n\nenum s(s) = p(p) | q(q)\n\ninputs\n  st(st) : s\n  x(x)   : bool\n\noutputs\n  nx(nx) : s\n  r(r)   : bool\n\ntable m(m)\npolicy unique\n| st | x     | -> nx(nx) : s | r(r) : bool |\n| p  | true  | q             | true        |\n| p  | false | p             | false       |\n| q  | -     | q             | false       |\n\nmachine k(k) over m\n  carry   st -> nx\n  held    r\n  initial p\n  final   q\n";
/// E058: a `derive` line with nothing after the word — the line that used to hang the parser.
const X_E058: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  r(r) : bool\n\nderive\n\ntable j(j)\npolicy unique\n| a     | -> r(r) : bool |\n| true  | false          |\n| false | true           |\n";
/// E059: two values with no operator between them, which used to read as the first alone.
const X_E059: &str = "rule t(t) v1\n\ninputs\n  a(a) : money[\u{5186}]  range >=0\u{5186} <=100\u{5186}\n  b(b) : money[\u{5186}]  range >=0\u{5186} <=100\u{5186}\n\noutputs\n  o(o) : money[\u{5186}]  round down(1\u{5186})\n\ndefine s(s) : money[\u{5186}] = a b\n\nresult o = s\n";
const X_E059_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY]  range >=0JPY <=100JPY
  b(b) : money[JPY]  range >=0JPY <=100JPY

outputs
  o(o) : money[JPY]  round down(1JPY)

define s(s) : money[JPY] = a b

result o = s
"##;
/// E060: a rounding grid of zero, which passed and divided by zero in the generated code.
const X_E060: &str = "rule t(t) v1\n\ninputs\n  a(a) : money[\u{5186}]  range >=0\u{5186} <=100\u{5186}\n\noutputs\n  o(o) : money[\u{5186}]  round up(0\u{5186})\n\nresult o = a\n";
const X_E060_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY]  range >=0JPY <=100JPY

outputs
  o(o) : money[JPY]  round up(0JPY)

result o = a
"##;
/// E061: a range whose ends are the wrong way round, which passed with nothing in it.
const X_E061: &str = "rule t(t) v1\n\ninputs\n  a(a) : money[\u{5186}]  range >=10\u{5186} <=0\u{5186}\n\noutputs\n  o(o) : money[\u{5186}]  round down(1\u{5186})\n\nresult o = a\n";
const X_E061_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY]  range >=10JPY <=0JPY

outputs
  o(o) : money[JPY]  round down(1JPY)

result o = a
"##;
/// E062: a date with the shape of one and no such day, which the checker counted on from.
const X_E062: &str = "rule t(t) v1\n\ninputs\n  d(d) : date  range >=2026-01-01 <=2026-12-31\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| d            | -> r(r) : bool |\n| <=2026-02-30 | true           |\n| >2026-02-30  | false          |\n";
/// E063: a comparison with something after it, which used to read as the comparison alone.
const X_E063: &str = "rule t(t) v1\n\ninputs\n  a(a) : money[\u{5186}]  range >=0\u{5186} <=1000\u{5186}\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| a             | -> r(r) : bool |\n| <=100\u{5186} + 1\u{5186} | true           |\n| >100\u{5186}        | false          |\n";
const X_E063_EN: &str = r##"rule t(t) v1

inputs
  a(a) : money[JPY]  range >=0JPY <=1000JPY

outputs
  r(r) : bool

table j(j)
policy unique
| a               | -> r(r) : bool |
| <=100JPY + 1JPY | true           |
| >100JPY         | false          |
"##;
/// E066 (§15.212): a file of enums that holds a table.
const X_E066: &str = "enums 区分(kinds) v1\n\n\
                      enum 区分(kind) = 冷蔵(chilled) | 常温(ambient)\n\n\
                      table 送料(fee)\npolicy unique\n| 区分 | -> 送料 : number |\n| -    | 1                |\n";
const X_E066_EN: &str = "enums kinds v1\n\n\
                      enum kind = chilled | ambient\n\n\
                      table fee\npolicy unique\n| kind | -> fee : number |\n| -    | 1               |\n";
/// E069 (§15.210): a comparison on an enum whose values are separated by `|`.
const X_E069: &str = "rule 会員送料(member_fee) v1\n\n\
                      enum 段階(tier) = 一般(basic) | シルバー(silver) | ゴールド(gold)\n\n\
                      inputs\n  会員(member) : 段階\n\n\
                      outputs\n  送料(fee) : money[円]  round down(1円)\n\n\
                      table 送料表(fees)\npolicy unique\n| 会員       | -> 送料 |\n| >=シルバー | 0円     |\n| 一般       | 500円   |\n";
const X_E069_EN: &str = "rule member_fee v1\n\n\
                      enum tier = basic | silver | gold\n\n\
                      inputs\n  member : tier\n\n\
                      outputs\n  fee : money[USD]  round down(1USD)\n\n\
                      table fees\npolicy unique\n| member   | -> fee |\n| >=silver | 0USD   |\n| basic    | 5USD   |\n";
/// E065: `range from` written in a shape that names no koyomi date.
const X_E065: &str = "rule t(t) v1\n\ninputs\n  d(d) : date  range from koyomi \"terms.cal\"\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| d | -> r(r) : bool |\n| - | true           |\n";
/// E129: a rule whose range is a koyomi date, checked where no koyomi is joined.
const X_E129: &str = "rule t(t) v1\n\ninputs\n  d(d) : date  range from koyomi \"terms.cal\" date payment\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| d | -> r(r) : bool |\n| - | true           |\n";
/// E130: the koyomi file has no such date.
const X_E130: &str = "rule t(t) v1\n\ninputs\n  d(d) : date  range from koyomi \"terms.cal\" date due\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| d | -> r(r) : bool |\n| - | true           |\n";
/// The koyomi file beside the examples of E129 and E130: the 10th of each month after a close on the 20th.
const X_TERMS_CAL: &str = "dates terms v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-20\n\ndate closing = received\n  close day 20\n\ndate payment = closing\n  day 10 of month +1\n";

/// E064: a row one cell short of its header, which used to be read as it stood.
const X_E064: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| a     | -> r(r) : bool |\n| true  |\n| false | true           |\n";
/// E057: an input with a name and nothing after it.
const X_E057: &str = "rule t(t) v1\n\ninputs\n  a(a) : bool\n  b(b)\n\noutputs\n  r(r) : bool\n\ntable j(j)\npolicy unique\n| a     | -> r(r) : bool |\n| true  | false          |\n| false | true           |\n";
const X_E124: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | a             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial a\n  final   b, c\n";
const X_E125: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c) | d(d)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | d             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n| d  | -   | d             |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial a\n  final   c\n";
const X_E126: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | a             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial a\n  final   c\n  never   c after b\n";
const X_E127: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n  f(f) : bool\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s | f(f) : bool |\n| a  | fwd | b             | false       |\n| a  | rev | a             | false       |\n| b  | fwd | c             | false       |\n| b  | rev | a             | true        |\n| c  | -   | c             | false       |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial a\n  final   c\n  once    f true\n";
const X_E128: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | a             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial a\n  final   c\n";
const X_W125: &str = "rule t(t) v1\n\nenum s(s) = a(a) | b(b) | c(c) | d(d)\nenum e(e) = fwd(fwd) | rev(rev)\n\ninputs\n  st(st) : s\n  ev(ev) : e\n\noutputs\n  nx(nx) : s\n\ntable m(m)\npolicy unique\n| st | ev  | -> nx(nx) : s |\n| a  | fwd | b             |\n| a  | rev | a             |\n| b  | fwd | c             |\n| b  | rev | a             |\n| c  | -   | c             |\n| d  | -   | d             |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial a\n  final   c\n";
const X_W126: &str = X_W125;
const X_W127: &str = "rule t(t) v1\n\nenum s(s) = p(p) | q(q)\n\ninputs\n  st(st) : s\n  a(a) : money[円]  range >=0円 <=10万円\n\noutputs\n  nx(nx) : s\n\nderive 倍(d) : money[円] = a + a  range >=0円 <=20万円\n\ndefine 上(up) : bool = 倍 >= 5円\ndefine 下(dn) : bool = 倍 <= 5円\n\ntable m(m)\npolicy first\n| st | 上   | 下   | -> nx(nx) : s |\n| p  | true | true | q             |\n| -  | -    | -    | st            |\n\nmachine k(k) over m\n  carry   st -> nx\n  initial p\n";
const X_W127_EN: &str = r##"rule t(t) v1

enum s(s) = p(p) | q(q)

inputs
  st(st) : s
  a(a) : money[JPY]  range >=0JPY <=100_000JPY

outputs
  nx(nx) : s

derive twice(d) : money[JPY] = a + a  range >=0JPY <=200_000JPY

define above(up) : bool = twice >= 5JPY
define below(dn) : bool = twice <= 5JPY

table m(m)
policy first
| st | above | below | -> nx(nx) : s |
| p  | true  | true  | q             |
| -  | -     | -     | st            |

machine k(k) over m
  carry   st -> nx
  initial p
"##;

// ── The ledger ───────────────────────────────────────────────────────────

/// Every code, in ledger order: syntax and names first (E001–E013), then the table checks
/// (E101–E113), then the warnings. There are no vacant numbers left.
/// The one fake key the reproductions of W901 hold (ritsu's DESIGN 16.10): a Google API key that
/// reads as a fake. It is put together from two pieces, so that no file of the source holds a key
/// in one run; the pages of codes that `explain` writes hold it whole.
macro_rules! fake_key {
    () => {
        concat!("AIzaSyD-ritsu-fake-", "key-for-tests-000000")
    };
}

const X_W901: &str = concat!(
    "rule 席数(seats) v1\n# 数を読んだときの Google Sheets の API キー: ",
    fake_key!(),
    "\n\nenum 種類(kind) = 講習(workshop) | 講演(talk)\n\ninputs\n  催しの種類(event_kind) : 種類\n\noutputs\n  席(needed) : number  round down(1)\n\ntable 選ぶ(pick)\npolicy unique\n| 催しの種類 | -> 席 : number |\n| 講習       | 30             |\n| 講演       | 80             |\n"
);
const X_W901_EN: &str = concat!(
    "rule seats v1\n# the Google Sheets API key the counts were read with: ",
    fake_key!(),
    "\n\nenum kind = workshop | talk\n\ninputs\n  event_kind : kind\n\noutputs\n  needed : number  round down(1)\n\ntable pick\npolicy unique\n| event_kind | -> needed : number |\n| workshop   | 30                 |\n| talk       | 80                 |\n"
);

/// The contract of the E074 examples: an order whose lines carry an amount, always there and
/// never more than ten of them, so that the one thing wrong is the shape of the `from`
/// (§15.223).
const AMOUNTS_DOC: &[(&str, &str)] = &[(
    "order.json",
    "{\"$defs\":{\"Order\":{\"type\":\"object\",\"required\":[\"lines\"],\"properties\":{\"lines\":{\"type\":\"array\",\"maxItems\":10,\
     \"items\":{\"type\":\"object\",\"required\":[\"amount\"],\"properties\":{\"amount\":{\"type\":\"integer\",\"minimum\":0,\"maximum\":1000}}}}}}}}\n",
)];
const X_E074: &str = "rule t(t) v1\n\nshape 注文(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  最高額(top) : money[円]  range >=0円 <=1000円  from max 注文.lines of amount\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| 最高額 | -> x |\n| -      | true |\n";
const X_E074_EN: &str = r##"rule t(t) v1

shape order(order) = jsonschema "order.json" "#/$defs/Order"

inputs
  top(top) : money[JPY]  range >=0JPY <=1000JPY  from max order.lines of amount

outputs
  x(x) : bool

table t1(t1)
policy unique
| top | -> x |
| -   | true |
"##;
// The merging of files (§15.227–§15.230). Each reproduction is the file checked, `explain.rule`,
// with the rule it adds to or reads beside it; every name is English.
const X_E067: &str = r##"contribution c(c) v1
to "base.rule" signature:9e889c8a59016488

extend kind with z
"##;
const E067_BASE: &[(&str, &str)] = &[("base.rule", r##"rule base(base) v1

enum kind(kind) = a(a) | b(b)

inputs
  k(k) : kind

outputs
  fee(fee) : number  round down(1)

table t(t)
policy unique
| k | -> fee : number |
| a | 1               |
| b | 2               |
"##)];
const X_E068: &str = r##"rule t(t) v1

inputs
  member(member) : bool

outputs
  bonus(bonus) : rate[step 1%]  round down(1%)

table p(p)
policy unique
| member | -> from_member : rate[step 1%] |
| true   | 10%                            |
| false  | 0%                             |

open bonus : rate[step 1%]  combine sum
  from_member
"##;
const X_E131: &str = r##"contribution c(c) v1
to "base.rule" signature:b10f6ca4271b3f2f

inputs
  promo(promo) : bool

table q(q)
policy unique
| promo | -> from_promo : rate[step 1%] |
| true  | 15%                           |
| false | 0%                            |

into bonus
  from_promo
"##;
const E131_BASE: &[(&str, &str)] = &[("base.rule", r##"rule base(base) v1

inputs
  member(member) : bool

outputs
  bonus(bonus) : rate[step 1%]  round down(1%)

table p(p)
policy unique
| member | -> from_member : rate[step 1%] |
| true   | 10%                            |
| false  | 0%                             |

open bonus : rate[step 1%]  combine sum  range >=0% <=20%
  from_member
"##)];
const X_E033_ADDED: &str = r##"contribution c(c) v1
to "base.rule" signature:69e01897ab91b191

extend kind with z
"##;
const E033_BASE: &[(&str, &str)] = &[("base.rule", r##"rule base(base) v1

enum kind(kind) = a(a) | b(b)  open

inputs
  k(k) : kind

outputs
  fee(fee) : number  round down(1)

table t(t)
policy first
| k | -> fee : number |
| a | 1               |
| - | 2               |
"##)];
const X_E040_SHAPE: &str = r##"contribution c(c) v1
to "base.rule" signature:0123456789abcdef

extend kind with z
"##;
const X_W132: &str = r##"rule t(t) v1

inputs
  plan(plan) : bool

outputs
  limit(limit) : number  round down(1)

table cap_of(cap_of)
policy unique
| plan  | -> plan_cap : number |
| true  | 40                   |
| false | 60                   |

open cap : number  combine max  empty 100  range >=0 <=100
  plan_cap

define limit : number = cap
"##;
const X_E044_RING: &str = r##"rule t(t) v1

inputs
  n(n) : number  range >=1 <=10

outputs
  y(y) : number  round down(1)

apply o(o) = "other.rule"
  a = n
  x -> y
"##;
const E044_OTHER: &[(&str, &str)] = &[("other.rule", r##"rule other(other) v1

inputs
  a(a) : number  range >=1 <=10

outputs
  x(x) : number  round down(1)

apply back(back) = "explain.rule"
  n = a
  y -> x
"##)];

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
                "その行のうちに `\"` を閉じてください。長い説明は一行に収めるか、`#` のコメントに移してください。",
                "Close the `\"` on the same line. A long description either fits on one line or moves into a `#` comment."
            ),
            X_E001,
            &["E002"],
        ),
        err(
            "E002",
            tr!("読めない文字があります", "Unreadable character"),
            tr!(
                "名前の位置に、文字でも `_` でもない文字（記号や制御文字）があるとき。括弧に入れた別名が名前になっていないとき（`六十(60)`）も同じです。打ち間違いが黙って名前になったり、別名が黙って消えたりするのを防ぐための検査です。",
                "A character that is neither a letter nor `_` appears where a name is expected — an alias in parentheses that is not a name (`sixty(60)`) included. The check exists so that a typo does not quietly become a name, nor an alias quietly go missing."
            ),
            tr!(
                "その文字を消してください。名前は文字か `_` で始まります（`@x(x)` なら `x(x)`、`六十(60)` なら `六十(size_60)`）。",
                "Delete the character. A name starts with a letter or `_` (`@x(x)` becomes `x(x)`, and `sixty(60)` becomes `sixty(size_60)`)."
            ),
            X_E002,
            &["E001", "E009"],
        ),
        err(
            "E003",
            tr!("ファイルが `rule` の行で始まっていません", "The file does not start with a `rule` line"),
            tr!(
                "一つの `.rule` は一つの規則で、先頭行が規則名と版です。空行とコメントより前に他の宣言があるとき、または `rule` の行に名前が無いとき。名前の無い `rule` の行は、いままで何も言われずに、中身の無い規則として検査を通っていました。",
                "One `.rule` is one rule, and its first line carries the name and the version. Anything else comes before it, or the `rule` line has no name — a file whose `rule` line had none used to pass the check as a rule with nothing in it."
            ),
            tr!(
                "先頭に `rule 規則名(alias) v1` の一行を足してください。",
                "Add `rule <name>(<alias>) v1` as the first line."
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
                "行頭に宣言の語を書いてください（`{}`）。表の行なら `|` で始めてください。",
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
                "行頭の語が語彙にないとき。語彙には同義の綴りがなく、英語の一種類だけです。",
                "The word at the head of the line is not in the vocabulary. The vocabulary has no synonyms: one English spelling each."
            ),
            tr!(
                "`{}` のどれかに直してください。業務の語を書けるのは、名前とセルの中だけです。",
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
                "`enum` `group` `derive` `define` `result` の行に、名前と中身を分ける `=` が無いとき。",
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
                "`policy` の後ろが {} 以外のとき。DMN の Any / Priority / Collect は採っていません。",
                "The word after `policy` is not {}. DMN's Any, Priority and Collect are not adopted.",
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
                "表のセルが空白だけのとき。**空欄は書き忘れと区別がつかない**ので構文エラーにしています。",
                "A cell of a table holds only whitespace. **A blank is indistinguishable from a forgotten entry**, so it is a syntax error."
            ),
            tr!(
                "どの値でもよいなら `-` と書いてください。値を書き忘れていたなら、その値を書いてください。",
                "If any value is meant, write `-`. If a value was forgotten, write the value."
            ),
            X_E008,
            &["E101"],
        ),
        err(
            "E009",
            tr!("宣言の名前が予約語と衝突しています", "A declared name collides with a reserved word"),
            tr!(
                "宣言した名前（や別名）が語彙の語と同じとき。行指向のパーサーがその行をセクションの始まりと読んで、宣言を黙って捨てる事故を防ぐための検査です。列挙の値は行の頭に来ないので、列挙の値で出るのは、値の位置で別の意味に読まれる 11 語（`not` `none` `true` `false` `starts_with` `default` `after` `empty` `exhausted` `by` `with`）のときだけです。",
                "A declared name (or alias) is the same as a word of the vocabulary. Otherwise the line-oriented parser reads the line as the start of a section and drops the declaration silently. An enum value never starts a line, so for a value it is only the eleven words its own position reads as something else (`not` `none` `true` `false` `starts_with` `default` `after` `empty` `exhausted` `by` `with`)."
            ),
            tr!(
                "名前を変えてください（`enum range(kind)` なら `enum 範囲区分(range_kind)`）。予約語は `src/kw.rs` の表ひとつで決まっています。",
                "Rename it (`enum range(kind)` becomes `enum range_band(range_kind)`). The reserved words are fixed by the one table in `src/kw.rs`."
            ),
            X_E009,
            &["E005", "E011"],
        ),
        err(
            "E010",
            tr!("`..` を使った範囲記法は書けません", "The `..` range notation is not allowed"),
            tr!(
                "セルに `0g..1000g` のような `..` があるとき。「1000g まで」が両端を含むのか含まないのかが、書いてある字から読めないためです。",
                "A cell contains `..`, as in `0g..1000g`. Whether \"up to 1000g\" includes the endpoint cannot be read off the text."
            ),
            tr!(
                "比較演算子で書き直してください。`0g..1000g` は `<=1000g` か `<1000g` のどちらかです。両端を決めたいなら `>=0g <=1000g` と並べて書いてください。",
                "Rewrite it with comparison operators: `0g..1000g` is either `<=1000g` or `<1000g`. To fix both ends, write `>=0g <=1000g`."
            ),
            X_E010,
            &["E105", "E101"],
        ),
        err(
            "E011",
            tr!("公開される名前に ASCII の別名がありません", "A public name has no ASCII alias"),
            tr!(
                "規則名・入力・出力・並び・要素のフィールドに、**ASCII でない名前**が付いていて括弧の中の別名も無いとき。別名は生成コードの公開名になります（漢字は大文字を持てず、Go の公開識別子になれません。`.proto` のフィールドの名前は ASCII だけです）。並びは呼ぶ側が渡す入力で、要素のフィールドは呼ぶ側が埋めるので、入力と同じく公開されます。名前がもとから ASCII なら、それ自身が公開名になるので別名は要りません。",
                "The rule name, an input, an output, a sequence or a field of one element has a **non-ASCII name** and no alias in parentheses. The alias becomes the public name in the generated code (a kanji has no uppercase and cannot begin an exported Go identifier, and the name of a field of the `.proto` is ASCII alone). A sequence is an input the caller passes, and the fields of one element are what the caller fills in, so they are as public as an input. A name that is already ASCII is its own public name and needs no alias."
            ),
            tr!(
                "括弧で別名を足してください（`重量 : bool` なら `重量(weight) : bool`）。導出・定義・グループ・表の別名は任意で、書けば生成コードがその名前を使い、書かなければ宣言した名前をそのまま使います。",
                "Add the alias in parentheses (`重量 : bool` becomes `重量(weight) : bool`). On a derived value, a definition, a group or a table the alias is optional: write one and the generated code uses it, leave it out and the declared name is used as it stands."
            ),
            X_E011,
            &["E009", "E012"],
        )
        .also(tr!("別名の無い並び", "A sequence with no ASCII alias"), X_E011_SEQ, X_E011_SEQ_EN),
        err(
            "E012",
            tr!("宣言されていない名前です", "The name is not declared"),
            tr!(
                "表の列や式が、どこにも宣言されていない名前を指しているとき。前に宣言していない名前は使えないので、名前はそれを使う行より上で宣言する必要があります。セルやグループに書いた値がどの列挙の値でもないときにも出ます。組み込みの区分（`std/us/states` など）を取り込んでいれば、注に、似た綴りの値か、その語を値に持つ別の国の名前空間が出ます。取り込んだ二つの国が同じ綴りの値を持つとき（`WA` はワシントン州と西オーストラリア州の符号です）は、その綴りをそのまま書いても出ます。`none` はこれに当たりません。optional の列では値の無いことで、optional でない列では E103 です。",
                "A column or an expression names something that is declared nowhere. There is no forward reference: a name is declared above the line that uses it. It also appears when a value written in a cell or a group is a value of no enum; when the rule imports a built-in namespace of divisions (`std/us/states` and the rest), the note gives the closest spelling, or the namespace of another country that has the word. And it appears when a word is a spelling two imported countries share (`WA` is the code of Washington and of Western Australia). `none` is not one of these: under an optional column it is the absent value, and under any other column it is E103."
            ),
            tr!(
                "綴りを宣言に合わせるか、その入力・導出・定義を上に宣言してください。二つの国が共有する綴りなら、値の名前（`Washington` か `Western_Australia`）で書いてください。",
                "Match the spelling to the declaration, or declare the input, derived value or definition above. For a spelling two countries share, write the value's name (`Washington` or `Western_Australia`)."
            ),
            X_E012,
            &["E011", "E013"],
        ),
        err(
            "E013",
            tr!("取込先がありません", "No such import"),
            tr!(
                "規則が外から読むものが無いか、読めないとき。外から読む行は六つあります。組み込みの名前空間を取り込む `import`（十三か国の一段目の区分。`std/us/states`、`std/gb/nations`、`std/cn/provinces`、`std/tw/divisions`、`std/kr/provinces`、`std/in/states`、`std/fr/regions`、`std/es/communities`、`std/it/regions`、`std/de/states`、`std/au/states`、`std/br/states`、`std/jp/prefectures`。日本の都道府県は `std/都道府県` でも取り込めて、そのときは値を日本語で書きます）、`.proto` の列挙を取り込む `import proto \"<ファイル>\" <列挙> -> <この規則の列挙>`、JSON Schema の列挙を取り込む `import jsonschema \"<ファイル>\" \"<ポインタ>\" -> <この規則の列挙>`（OpenAPI も同じ）、`enums` のファイルの列挙を取り込む `import enum <列挙> from \"<ファイル>\"`、入力を読む契約を名付ける `shape <名前> = proto \"<ファイル>\" <メッセージ>`（JSON Schema なら `jsonschema \"<ファイル>\" \"<ポインタ>\"`）と入力の `from`、規則を契約のサービスとして出す `implements proto \"<ファイル>\" <サービス>` です。ファイルを読む行では、ファイルを読めないとき、指した列挙・メッセージ・サービスがファイルに無いとき、行の形が違うときに出ます。`implements` の行が二行あるときにも出ます。rulec は YAML を読まないので、JSON にしたものを指してください。一つの国の区分を二つの名前で取り込んだとき（`std/都道府県` と `std/jp/prefectures`）にも出ます。",
                "Something the rule reads from outside is not there, or cannot be read. Six lines read from outside: an `import` of a built-in namespace (the first-level divisions of thirteen countries: `std/us/states`, `std/gb/nations`, `std/cn/provinces`, `std/tw/divisions`, `std/kr/provinces`, `std/in/states`, `std/fr/regions`, `std/es/communities`, `std/it/regions`, `std/de/states`, `std/au/states`, `std/br/states` and `std/jp/prefectures`, whose prefectures `std/都道府県` also imports, with the values spelled in Japanese); `import proto \"<file>\" <Enum> -> <enum of this rule>`, an enum of a `.proto`; `import jsonschema \"<file>\" \"<pointer>\" -> <enum of this rule>`, an enum of a JSON Schema, OpenAPI included; `import enum <enum> from \"<file>\"`, an enum of an `enums` file; `shape <name> = proto \"<file>\" <Message>` (or `jsonschema \"<file>\" \"<pointer>\"`), the contract an input's `from` reads; and `implements proto \"<file>\" <Service>`, the contract's service the rule is served as. For a line that reads a file it appears when the file cannot be read, when the enum, message or service it names is not in the file, or when the line is not that shape; and when there are two `implements` lines. YAML is not read; point at a JSON form of it. It also appears when one country's divisions are imported under both of its names (`std/都道府県` and `std/jp/prefectures`)."
            ),
            tr!(
                "名前空間の綴りを直すか（似た綴りがあれば注に出ます）、その列挙を `enum` でこのファイルに書いてください。ファイルを読む行では、パスは規則ファイルのある場所からの相対パスで書いてください。列挙・メッセージ・サービスが見つからないときは、注に、そのファイルにあるものが並びます。メッセージとサービスは、名前か、ファイルの package を付けた名前で書いてください。JSON Schema のポインタは `#/components/schemas/<名前>` の形です。ポインタが届かなかったときは、注に、届いたところにある鍵が並びます。`implements` は一つの規則に一行です。",
                "Correct the namespace's spelling (the note gives the closest one), or declare the enum in this file with `enum`. For a line that reads a file, the path is followed from the directory of the rule file, so write it relative to that. An enum, a message or a service that is not found comes back with the ones the file has, and a message or a service is named by its name or by its name with the file's package. A JSON Schema pointer looks like `#/components/schemas/<name>`, and one that does not resolve comes back with the keys that are there. A rule has one `implements` line."
            ),
            X_E013,
            &["E012", "E032", "E076"],
        )
        .english(X_E013_EN),
        err(
            "E014",
            tr!("出力のセルに式は書けません", "An output cell cannot hold an expression"),
            tr!(
                "表の `->` から右のセルに語が二つ以上あるとき。書けるのは値一つか名前一つだけです。残りを読み飛ばして最初の語だけを採ると、かけ算が黙って消えた生成コードができてしまいます。",
                "A cell to the right of `->` holds two or more words. Only one value or one name may be written there. Taking just the first word and skipping the rest would emit generated code with the multiplication silently dropped."
            ),
            tr!(
                "計算に名前を付けて `define` の行へ出し、表にはその名前だけを書いてください（`| - | 率割引 |`）。表に書くのは分岐だけです。",
                "Give the calculation a name on a `define` line and leave only that name in the table (`| - | rate_discount |`). A table holds the branching and nothing else."
            ),
            X_E014,
            &["E008", "E012"],
        )
        .english(X_E014_EN),
        err(
            "E015",
            tr!("`result` が書けるのは最初の出力だけです", "`result` can only assemble the first output"),
            tr!(
                "`result` が二つ目以降の出力を指したとき。`result` は最初の出力を組み立てるための書き方で、評価器も生成コードもそこにしか当てません。書いた名前が効かないまま通っていたので、`number` が `money` の枠に入っても E103 が出ませんでした。",
                "A `result` names an output other than the first. `result` is sugar for the first output, and both the evaluator and the generated code apply it only there. The name used to be ignored, so a `number` could land in a `money` slot without an E103."
            ),
            tr!(
                "その出力と同じ名前の `define` を書いてください（`define 付与点(pts) : number = 基本点 × 倍率`）。出力の値は宣言の順に、同じ名前の `define` から取られます。`result` で組み立てたいなら、その出力を `outputs` の先頭へ移してください。",
                "Write a `define` of the same name as that output (`define points(pts) : number = base_points * multiplier`); outputs are taken, in declaration order, from the binding of their own name. To assemble it with `result` instead, move that output to the top of `outputs`."
            ),
            X_E015,
            &["E016", "E103"],
        )
        .english(X_E015_EN),
        err(
            "E016",
            tr!("`result` は一つしか書けません", "There can be only one `result`"),
            tr!(
                "一つのファイルに `result` の行が二本以上あるとき。組み立てられるのは最初の出力だけなので、二本目は一本目を置き換えるだけになります。以前はそれが黙って起きていました。",
                "A file has more than one `result` line. Only the first output can be assembled, so a second `result` merely replaces the first — which it used to do in silence."
            ),
            tr!(
                "一本だけ残してください。ほかの出力の値は、その出力と同じ名前の `define` から取られます。",
                "Keep one. The other outputs are taken from a `define` of the same name as the output."
            ),
            X_E016,
            &["E015"],
        )
        .english(X_E016_EN),
        err(
            "E017",
            tr!("`constraint` の形が違います", "A `constraint` is not shaped like this"),
            tr!(
                "`constraint` の行が「入力 比較 入力」になっていないとき。比較が無い、片側が名前一つでも名前を `+` で足した和でもない、両側とも和になっている、のどれかです。",
                "A `constraint` line is not `input comparison input`: there is no comparison, a side is neither one name nor names added up with `+`, or both sides are sums."
            ),
            tr!(
                "`constraint <入力> <= <入力>` の形にしてください。比較は `<=` `<` `>=` `>` の四つです。片側だけは入力を足した和にできます（`constraint 値引き + ポイント <= 小計`）。係数と定数は書けません。`A = B` を言いたいなら、`A <= B` と `A >= B` の二行に分けてください。",
                "Write `constraint <input> <= <input>`; the comparisons are `<=`, `<`, `>=` and `>`. One side may add inputs up (`constraint discount + points <= subtotal`); a coefficient or a constant cannot be written. For `A = B`, write the two lines `A <= B` and `A >= B`."
            ),
            X_E017,
            &["E018"],
        ),
        err(
            "E018",
            tr!("`constraint` は入力どうしの関係です", "A `constraint` relates two inputs"),
            tr!(
                "`constraint` の片側が入力でないか、順序の無い型のとき。制約は「呼び出し側が渡す値の組み合わせのうち、どれが起きるか」を言うものなので、両側とも `inputs` の名前で、型は金額・数量・率・number・日付のどれかでなければなりません。和の側の名前も、一つずつ同じです。",
                "A side of a `constraint` is not an input, or has a type with no order. A constraint says which combinations of the values the caller passes can happen, so both sides name something in `inputs`, and each is money, a quantity, a rate, a number or a date. Every name a sum adds is held to the same."
            ),
            tr!(
                "両側を `inputs` の名前にしてください。導出や定義は入力から計算されるので、関係は元の入力どうしで書いてください。列挙や真偽に大小はありません。",
                "Name inputs on both sides. A derived or defined value is computed from inputs, so write the relation between those inputs; an enum and a boolean have no order."
            ),
            X_E018,
            &["E017", "W111"],
        ),
        err(
            "E019",
            tr!("例が、生成コードが入口で受け付けない入力です", "An example is an input the generated code refuses at its door"),
            tr!(
                "例の入力が `constraint` を満たしていないとき、または入力（並びの要素のフィールドを含む）の値が宣言した範囲の外にあるとき。制約は「この組み合わせは起きない」という宣言で、範囲は「この外の値は来ない」という宣言です。完全性の検査はそれを信じて、そこには行を要求していません。生成コードもその入力を入口で受け付けません。そのため、規則はその入力の答えを決めていません。範囲の外の例と、二つの日付のあいだの `constraint` を破る例は、いままで検査を通り、その例から作ったベクタは、生成コードでエラーになっていました。",
                "An example's inputs do not satisfy a `constraint`, or the value of an input — a field of an element of the sequence included — lies outside its declared range. A constraint declares that a combination does not happen and a range that no value outside it arrives; the completeness check believed them and demanded no row there, and the generated code refuses that input at the door. It is not an input an answer can be claimed for. An example outside the range, and one that breaks a `constraint` between two dates, used to pass the check, and the vector made from it was then refused by the generated code."
            ),
            tr!(
                "例の値を直してください。その値や組み合わせが本当に来るなら、範囲を広げるか、制約を消してください。",
                "Correct the example's values — or, if that value or combination really does arrive, widen the range or drop the constraint."
            ),
            X_E019,
            &["E017", "E018", "E101"],
        ),
        err(
            "E020",
            tr!("`elements` の宣言が正しくありません", "The `elements` declaration is not right"),
            tr!(
                "`elements` に名前が無いとき、または宣言した並びをどの行もたどらないとき。並びは、`count`・`sum`・`max`・`min`・`fold` のどれかが `over` で指して、はじめて規則の値になります。たどらない並びのフィールドを読む項目は、どの要素の値を読むのかが決まりません。規則は並びをいくつでもたどれます（`elements` は何本でも書けます）。",
                "An `elements` line has no name, or nothing walks the sequence it declares. A sequence becomes a value of the rule only when a `count`, a `sum`, a `max`, a `min` or a `fold` names it after `over`; an item that reads a field of a sequence nothing walks reads no element in particular. A rule may walk any number of sequences, each under an `elements` of its own."
            ),
            tr!(
                "`elements 運賃行(fee_rows)` の形で名前を書き、続く行に一要素ぶんのフィールドを `inputs` と同じように書いてください。並びが要るなら、たどる行を足してください。要らないなら、その `elements` を消してください。",
                "Write `elements fee_rows(fee_rows)`, and the fields of one element under it, declared the way `inputs` are. If the sequence is needed, add the line that walks it; if not, take the `elements` out."
            ),
            X_E020,
            &["E028", "E021", "E031"],
        ),
        err(
            "E021",
            tr!("`fold` の書き方が正しくありません", "The `fold` is not written correctly"),
            tr!(
                "`fold <判定の列> over <並びの名前> [-> <名前> [: <型>]]` になっていないか、判定の行き先が `next` `stop` `stop with <値>` `take_unique <値>` `take_first <値>` `keep_max <値> by <鍵>` `take_weighted <値> by <重み>` のどれでもないか、畳もうとしている列が列挙でないとき。答えの名前が、入力、要素のフィールド、たどる並び、上で宣言した値と同じとき、出力でない名前に型が無いとき、出力の名前に出力と違う型を書いたとき、畳み込みとほかの項目が同じ名前を決めるときも、これです。畳み込みの値が、その値を決める時点に無い値を読むときもこれです。`empty` と `exhausted` の値は要素を見ていないところで決まるので、要素の値を読めません。どの値も、この畳み込みの答えから計算する値を読めません（要素の判定がその答えを読むと、答えを出す前に答えが要ります）。`held` は最後まで見終えたときの値なので、`exhausted` の値にだけ書けます。",
                "The heading is not `fold <verdict column> over <sequence> [-> <name> [: <type>]]`, or an arm is not one of `next`, `stop`, `stop with <value>`, `take_unique <value>`, `take_first <value>`, `keep_max <value> by <key>`, `take_weighted <value> by <weight>`, or the column being folded is not an enum. So is an answer named after an input, a field of an element, a sequence or a value declared above; a name that is no output given no type; an output's name given a type other than the output's; and a name that the fold and another item both decide. So, too, is a value of the fold that reads what is not there when it is decided: `empty` and `exhausted` are decided where no element is being looked at and cannot read an element's value; no value can read what is computed from the fold's own answer (a verdict that read it would need the answer before there is one); and `held`, what the walk holds once it has seen every element, belongs in `exhausted` only."
            ),
            tr!(
                "見出しと行き先を上の形に直してください。`take` とだけ書くことはできません。**一件だけ採るのか、最初の一件を採るのか**は、書く人が選ぶことだからです。要素の値を答えにするなら、`take_unique`・`take_first`・`keep_max` で採ってから、`exhausted -> held` で返してください。後ろの項目が答えを読むなら `-> <名前>` で名前を付けます。自分の答えを読む判定は、別の畳み込みにすれば二回目の歩きで読めます。",
                "Correct the heading and the arms. A bare `take` cannot be written: whether **one and only one** element may be taken, or the first of several, is for the author to choose. To answer with a value of an element, take it with `take_unique`, `take_first` or `keep_max`, and answer `exhausted -> held`. Name the answer with `-> <name>` for the items below to read it; a verdict that needs a fold's answer goes in another fold, which reads it on a second walk."
            ),
            X_E021,
            &["E020", "E022", "E023", "E024"],
        ),
        err(
            "E022",
            tr!("要素がゼロ件のときの答えが宣言されていません", "The answer for a sequence with no elements is not declared"),
            tr!(
                "`fold` に `empty -> <値>` が無いとき、`max`・`min` に `empty <値>` が無いとき。空の並びは必ず来ます。手で書いたループがいちばんよく見落とすのがこの場合で、たいていは最初の要素をそのまま読もうとして、実行時に落ちます。`max`・`min` では、`where` に当てはまる要素が一つも無いときも、この値が答えです。",
                "A `fold` has no `empty -> <value>`, or a `max` or a `min` has no `empty <value>`. An empty sequence always turns up, and it is the case a hand-written loop most often forgets — usually by reading the first element and falling over. For a `max` and a `min`, it is also the answer when no element passes the `where`."
            ),
            tr!(
                "`empty -> <値>`（`max`・`min` では `empty <値>`）を足してください。何を返すかは業務の判断で、rulec が決められることではありません。",
                "Add `empty -> <value>` (`empty <value>` on a `max` or a `min`). What to answer is a business decision, and not one the tool can make."
            ),
            X_E022,
            &["E023", "E024"],
        )
        .english(X_E022_EN)
        .also(tr!("`max` に `empty` が無い", "A `max` with no `empty`"), X_E022_MAX, X_E022_MAX),
        err(
            "E023",
            tr!("最後まで見終えたときの答えが宣言されていません", "The answer for a walk that reached the end is not declared"),
            tr!(
                "`fold` に `exhausted -> <値>` が無いとき。どの要素も打ち切らずに並びが尽きた場合の答えです。保持していた暫定の値をそのまま返すつもりでも、書くまでは決まっていません。",
                "A `fold` has no `exhausted -> <value>`: the answer when the sequence ran out and no element ended the walk. Answering with the value that was held is a choice, and it is made by writing it."
            ),
            tr!(
                "`exhausted -> <値>` を足してください。保持しているものを返すなら `exhausted -> held` です。",
                "Add `exhausted -> <value>`; to answer with what is held, that is `exhausted -> held`."
            ),
            X_E023,
            &["E022", "E024"],
        )
        .english(X_E023_EN),
        err(
            "E024",
            tr!("行き先の無い判定があります", "Some verdict has no arm"),
            tr!(
                "表が出しうる判定のどれかに、`fold` の行き先が無いとき。その判定の要素が来たら、次にどうするかが決まっていません。表の完全性と同じ検査を、畳み込みの側に当てたものです。",
                "A verdict the table can produce has no arm in the `fold`: when an element lands on it, the walk has no move. It is the table's own completeness check, applied to the fold."
            ),
            tr!(
                "行き先を足すか、表がその値を出さないようにしてください。逆に、どの要素もたどり着けない判定に行き先があるときは、W115 が出ます。",
                "Add the arm, or stop the table producing that value. The other direction — an arm for a verdict nothing can reach — is W115."
            ),
            X_E024,
            &["E022", "E023", "W115"],
        )
        .english(X_E024_EN),
        err(
            "E025",
            tr!("例に並びの列がありません", "The examples have no column for the sequence"),
            tr!(
                "並びをたどる規則に `examples` があるのに、`elements` の名前の列が見出しに無いとき。どの並びをたどるかが決まっていない例は、答えの決まっていない例です。",
                "A rule that walks a sequence has `examples`, but the header has no column named after its `elements`. An example that does not say which sequence it walks is an example with no answer."
            ),
            tr!(
                "`sequence <名前>` で並びを書き、例の見出しに並びの列を足して、その名前をセルに書いてください。行がゼロ本の `sequence` は、要素ゼロ件の例になります。",
                "Write the list with `sequence <name>`, add a column for the sequence to the examples header, and name it in the cell. A `sequence` with no rows is the example for a sequence with nothing in it."
            ),
            X_E025,
            &["E026", "E027"],
        )
        .english(X_E025_EN),
        err(
            "E026",
            tr!("`sequence` の書き方が正しくありません", "The `sequence` is not written correctly"),
            tr!(
                "`sequence` のフィールドが `elements` のフィールドとそろっていないとき。余分なフィールドがある、フィールドが足りない、見出しに同じフィールドの列が二つある、`->` がある、たどる並びそのものが無い、同じ名前が二つある、セルが値でない（範囲や `-` が書いてある）、のどれかです。",
                "The columns of a `sequence` do not line up with the fields of `elements`: a column that is not a field, a field left out, one field's column twice, a `->`, no sequence to be a list of, two blocks with the same name, or a cell that is not a value (a range or a `-`)."
            ),
            tr!(
                "`elements` のフィールドをそのまま見出しにして、一行に一件ぶんの値を書いてください。これは表ではなく、実際に渡す値の並びです。",
                "Make the header the fields of `elements` as they are, and write one element's values per row. This is not a table: it is the list of values as they would really be passed."
            ),
            X_E026,
            &["E025", "E020"],
        )
        .english(X_E026_EN),
        err(
            "E027",
            tr!("例が指す並びがありません", "The example names a sequence that is not there"),
            tr!(
                "例の並びの列に書かれた名前の `sequence` が無いとき、またはその列に名前でないもの（数や範囲）が書かれているとき。",
                "The cell in the sequence column names a `sequence` that is not declared, or holds something that is not a name at all."
            ),
            tr!(
                "その名前で `sequence` を書くか、セルを、書いてある `sequence` の名前に直してください。",
                "Write a `sequence` under that name, or correct the cell to one that is written."
            ),
            X_E027,
            &["E025", "E026"],
        )
        .english(X_E027_EN),
        err(
            "E028",
            tr!("`count` の書き方が正しくありません", "The `count` is not written correctly"),
            tr!(
                "`count <名前>(<別名>) over <並びの名前> where <列> = <値>` になっていないとき。`over` が無い、`where` が無い、指した並びが `elements` で宣言されていない、`=` の右に値が無い。`sum <名前>(<別名>) over <並びの名前> of <列>`、`max`・`min` の `<名前>(<別名>) over <並びの名前> of <列> [where <列> [= <値>]] empty <値>` の形でないときも、これです。",
                "The line is not `count <name>(<alias>) over <sequence> where <column> = <value>`: no `over`, no `where`, a sequence that `elements` does not declare, or an `=` with nothing on its right. So is a `sum` that is not `sum <name>(<alias>) over <sequence> of <column>`, and a `max` or a `min` that is not `<name>(<alias>) over <sequence> of <column> [where <column> [= <value>]] empty <value>`."
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
                "`where` が指す列が、要素ごとに決まる値でないとき（入力や導出は一件の呼び出しに一つしかないので、数えても 0 か 1 です）。値が有限の集合でないとき。書いた値がその列挙にないとき。列挙の列なのに `= <値>` が無いとき。`sum`・`max`・`min` の `of` が指す列も同じで、`over` が指すのと別の並びの値のとき、`sum` の列が数でないとき、`max`・`min` の列が数・日付・順序つきの列挙のどれでもないときにも出ます（`max`・`min` では題が「この列はまとめられません」になります）。",
                "The column `where` names is not a value of one element (an input or a derived value is one per call, so counting it could only answer 0 or 1); or its values are not a closed set; or the value written is not one of that enum\'s; or an enum column was given no `= <value>`. The column after `of` on a `sum`, a `max` or a `min` is held the same way, and it also appears when that column is a value of another sequence than the one `over` names, when a `sum` reads no number, and when a `max` or a `min` reads neither a number, a date nor an ordered enum (their title then says the column cannot be summarised)."
            ),
            tr!(
                "要素のフィールドか、要素ごとの表が出した列を指してください。判定を表に書けば、その分類そのものも完全性の検査に掛かります。",
                "Name a field of an element, or a column a per-element table produces. Writing the classification as a table is what puts the classification itself under the completeness check."
            ),
            X_E029,
            &["E028", "E012"],
        ),
        err(
            "E030",
            tr!("`count` に範囲が要ります", "A `count` needs a range"),
            tr!(
                "`count` の行に `range >=0 <=<上限>` が無いか、上限が無いか、下限が負のとき。範囲には二つの意味があります。数えた結果を列に使ったときに完全性の検査が調べる値の全体と、**並びの長さの上限**です。",
                "The `count` line has no `range >=0 <=<max>`, or no upper bound, or a negative lower one. The range means two things: the universe the completeness check quantifies over once the count is a column, and **the cap on the sequence**."
            ),
            tr!(
                "`range >=0 <=100` の形で書いてください。生成コードは、この上限より長い並びを入口で受け付けません。数値の入力と同じで、宣言の外は黙って通しません。",
                "Write it as `range >=0 <=100`. The generated code refuses a longer sequence at the door, the way it refuses a number outside its range."
            ),
            X_E030,
            &["E028", "E112"],
        ),
        err(
            "E031",
            tr!("一つの項目が、二つの並びの要素を読んでいます", "One item reads the elements of two sequences"),
            tr!(
                "一つの項目（表・節・導出・定義・畳み込み）が、二つの並びの要素の値を読むとき。要素の値は、その並びをたどっているあいだにしかありません。二つの並びの要素を一緒に読むには、片方の並びの要素ごとに、もう片方を最初からたどることになります（並びの中で並びをたどる）。そうすると歩きの数が並びの長さにつれて増え、検査が終わりません。出力が要素ごとの値のときも、これです。たどり終えたあとには、一つの値に決まりません。規則の項目は、要素によらない項目と要素ごとの項目に分かれ、要素ごとの項目は、読む集計の答えが決まったあとの歩きで計算されます（集計の答えを読む要素ごとの項目は二回目の歩きに入ります）。",
                "One item (a table, a clause, a derive, a definition or a fold) reads a value of one element of each of two sequences. A value of one element exists only while its sequence is walked, so reading both together would walk one sequence again from the start at every element of the other — a walk inside a walk. The number of walks would then grow with the length of a sequence, and the checks could not finish. So is an output that is a value of one element: once the walk is over, it is not one value. A rule's items fall into those that read no element and those of one element, and an item of one element is worked out on the walk after the one that finds every summary it reads (an item of one element that reads a summary goes on a second walk)."
            ),
            tr!(
                "片方の並びを `count`・`sum`・`max`・`min`・`fold` で一つの値にしてから、もう片方の項目で読んでください。その項目は、その値が決まったあとの二回目の歩きで計算されます。要素ごとの値を出力にするなら、集計で並び全体の一つの値にしてから出力にしてください。",
                "Make one of the sequences one value first — a `count`, a `sum`, a `max`, a `min` or a `fold` — and read that value in the item of the other; the item is then worked out on a second walk, once the value is known. To answer with something each element decides, make it one value for the whole sequence with a summary first."
            ),
            X_E031,
            &["E021", "E029", "E020"],
        )
        .also(tr!("要素ごとの値の出力", "An output that is a value of one element"), X_E031_OUTPUT, X_E031_OUTPUT_EN),
        err(
            "E032",
            tr!("取り込んだ列挙と宣言がずれています", "The declared enum and the imported one disagree"),
            tr!(
                "`import proto` や `import jsonschema` が指す列挙の値と、この規則の `enum` の値（別名があれば別名）がそろっていないとき。片方にしか無い値は、どちらの側のものも名前で出ます。値が増えるのはたいてい proto の側で、それは**規則の外での変更が、互換な変更として通ってしまったもの**です。",
                "The values of the enum named by `import proto` or `import jsonschema` and the values of the rule's `enum` (their ASCII aliases, where they have them) are not the same set. Values on either side alone are named, in both directions. It is usually the proto that gained one, and **it shipped as a compatible change made outside this rule**."
            ),
            tr!(
                "増えた値をこの規則の `enum` に足してください。値の名前に別名を付けて書く列挙なら、名前は自分で決めてください。消えた値なら、この規則からも消してください。値を足すと、次はその値に行が要るかを E033 が問います。",
                "Add the new value to the rule's `enum`; where the enum gives its values names with aliases, the name is yours to choose. A value the contract dropped goes from the rule too. Once it is added, the table asks whether it needs a row (E033)."
            ),
            X_E032,
            &["E013", "E033", "E101"],
        )
        .with_files(TIER_PROTO),
        err(
            "E033",
            tr!("外から来た列挙の値に、行も `default` もありません", "A value that came from elsewhere has neither a row nor `default`"),
            tr!(
                "取り込んだ列挙の値が、どの行にも現れず、`default` も付いていないとき。寄与が `extend` で足した値が、表のどの行にも名前で書かれず、`-` の行、その値を外さない `not:` の行、その値まで覆うようになった比べるセルの行に黙って落ちるときも同じです（行ごとに言います）。自分で書いた値なら書き忘れの警告（W111）で済みますが、契約や寄与から来た値の場合は**外の変更がまだ誰にも読まれていない**ことを示すので、rulec はエラーにして止めます。既定の行がある表では完全性の検査が通ってしまい、新しい値には既定の行の答えが黙って当たります。",
                "A value of an imported enum appears in no row and is not marked `default`. The same holds for a value a contribution adds with `extend` that no row of a table names and that falls in silence to a row of `-`, a `not:` that does not leave it out, or a comparing cell that now covers it (each such row is named). For a value you wrote yourself that is a forgotten line (W111); for a value that came through a contract or a contribution it means **a change from elsewhere that nobody has read yet**, so it stops. With a default row the completeness check passes and the new value quietly takes the default row's answer."
            ),
            tr!(
                "その値の行を表に足すか、値の宣言に `default` を付けてください。寄与が足した値なら、開いた表に寄与が行を足すか、`extend` の行に `default` を付けてください。`default` は「既定の行に落ちるのが意図です」という宣言で、誰かがその値の扱いを決めたことを示します。",
                "Add a row for it, or mark the value `default` in the declaration. For a value a contribution adds, have a contribution add its row to the open table, or put `default` on the `extend` line. `default` is a signature saying that falling through to the default row is what is meant — that someone decided how it is treated."
            ),
            X_E033,
            &["E032", "W111", "E101", "E067"],
        )
        .with_files(TIER_PROTO)
        .also_with(tr!("寄与が足した値（`extend`）", "A value a contribution adds (`extend`)"), X_E033_ADDED, E033_BASE),
        err(
            "E034",
            tr!("同じ名前が二度あります", "A name appears twice"),
            tr!(
                "一つのファイルが同じ種類の名前を二度書いたとき、二つ目のところで出ます。種類は、値（入力、たどる並びとその要素のフィールド、出力、`derive`・`define`・`count`・`sum`・`max`・`min`・`open` と、表や節が決める出力の列）、表と節、列挙、一つの列挙の値、グループと一つのグループに書いた値、`shape`、出典、準用、前提のつながり、一つの表の行ラベル、表と例の見出しの列です。出力はそれを決めるものと同じ名前で、一つの名前をいくつかの表や節で決めることもできます。値のそれ以外の重なりは、入力と同じ名前の出力も含めて、一つの名前に二つの値があることになるので止まります。列挙の値と同じ名前のグループも、一つの語に二つの読み方があるので止まります。一つの並びの二つの名前が生成するコードで同じ識別子になるとき（一方の別名が、他方の別名か名前と同じ）、一つの名前に二つの別名を書いたとき、`enums` のファイルで列挙の名前がほかの列挙の別名と同じとき（`import enum` は名前でも別名でも列挙を探します）にも出ます。二つの名前の識別子が違っても、それを書くどの言語でも同じに綴るときにも出ます。二つの列挙の型と一つの列挙の二つの値（どちらも PascalCase で書きます）、二つの前提のつながりの関数（snake_case）、並びが二つ以上ある規則の要素の型、呼ぶ側のオブジェクトから入力を読む関数の引数として並ぶ `shape` と入力や並びです。いくつかの言語だけが同じに綴るものは W121 です。`import enum` で読んだ列挙と一緒に来たグループはこの規則の名前やほかの `import enum` が読んだものと、寄与が足す入力は足す先の規則の名前と比べます。畳み込みの答えの名前は E021、準用の出力に付けた名前は E041 で止まります。",
                "A file writes one name twice among the names of one kind, and it is said at the second. The kinds are the values (the inputs, the sequences and the fields of their elements, the outputs, and what `derive`, `define`, `count`, `sum`, `max`, `min`, `open` and the output columns of tables and clauses decide), the tables and clauses, the enums, the values of one enum, the groups and the values one lists, the shapes, the sources, the applies, the prerequisites blocks, the labels of the rows of one table, and the columns of the heading of a table or of the examples. An output shares its name with what decides it, and several tables and clauses may decide one name; any other two values of one name stop here, an output named like an input among them. A group named like a value of an enum is one word read two ways, and stops here too. So do two names of one list that come to one identifier in the generated code (the alias of one is the alias or the name of the other), one name given two aliases, and, in an `enums` file, an enum's name that is another enum's alias (`import enum` finds an enum by either). So do two names whose identifiers differ but which every language that writes them spells as one: the types of two enums and two values of one enum (both written in PascalCase), the functions of two prerequisites blocks (in snake_case), the element types of a rule's two sequences, and a shape beside an input or a sequence among the parameters of the function that reads the caller's objects. Two that only some languages spell as one are W121. An enum read with `import enum`, and a group that comes with it, is held to the names of this rule and of the other `import enum` lines, and an input a contribution adds to the names of the rule it adds to. A fold's answer is held by E021, and the name an `apply` gives a callee output by E041."
            ),
            tr!(
                "どちらかを消すか、名前を変えてください（行ラベルと見出しの列も同じです）。同じ識別子になる二つの名前は、どちらかに別の別名を付けてください。`import enum` で読んだ列挙なら、`-> <別の名前>` で読めます。",
                "Remove one of the two, or rename one (a row label and a column of a heading alike). Of two names that come to one identifier, give one another alias. An enum read with `import enum` can be read under another name with `-> <another name>`."
            ),
            X_E034,
            &["E009", "E035", "E021", "E041", "E075", "W121"],
        )
        .english(X_E034_EN)
        .also(tr!("一つの列挙に同じ値が二つ", "One value written twice in an enum"), X_E034_VALUE, X_E034_VALUE_EN)
        .also(tr!("同じ識別子になる二つの名前", "Two names that come to one identifier"), X_E034_ALIAS, X_E034_ALIAS_EN)
        .also(tr!("表の見出しに同じ列が二つ", "One column twice in a table's heading"), X_E034_COLUMN, X_E034_COLUMN_EN)
        .also(tr!("型の名前が一つになる二つの列挙", "Two enums whose types take one name"), X_E034_TYPE, X_E034_TYPE_EN)
        .also(tr!("一つの表の二つの行に同じラベル", "One label on two rows of a table"), X_E034_LABEL, X_E034_LABEL_EN),
        err(
            "E035",
            tr!("`overrides` の指す先がありません", "The target of `overrides` does not exist"),
            tr!(
                "`overrides` が指す表が無いか、この表より後ろで宣言されているか、`表:行ラベル` の行にそのラベルが無いとき。行の書き方が読めないときも同じです。例外は本文の後に書くので、指す先はいつも上にあります。",
                "An `overrides` line names a table that does not exist, or one declared below this table, or a row label (`table:label`) the table has no row of. A line whose shape cannot be read is reported the same way. The exception is written after what it excepts, so a target is always above."
            ),
            tr!(
                "上で宣言した表か、その行（行の先頭にラベルを書き、`表:ラベル` で指す）を指定してください。優先する側を後に書いてください。",
                "Name a table declared above, or one of its rows (label the row at its head and write `table:label`). Write the side that takes precedence later."
            ),
            X_E035,
            &["E034", "E036"],
        )
        .english(X_E035_EN),
        err(
            "E036",
            tr!("`overrides` の相手が同じ出力を定めていません", "The target of `overrides` does not define the same output"),
            tr!(
                "`overrides` の指す表が、この表とは別の出力を定めているとき。優先の順序は、同じ出力を定める定義のあいだにだけあります。",
                "The table an `overrides` line names defines a different output from this table. Precedence exists only between definitions of the same output."
            ),
            tr!(
                "同じ出力を定める表を指すか、この表の出力列をその相手と揃えてください。",
                "Name a table that defines the same output, or make this table's output column the same one."
            ),
            X_E036,
            &["E035", "E045"],
        )
        .english(X_E036_EN),
        err(
            "E037",
            tr!("引用した箇所のハッシュが固定されていません", "A cited fragment is not pinned"),
            tr!(
                "`@出典 第91条` のように引用した箇所に、`source` の行の下の `  第91条 sha256:…` というハッシュの行が無いとき。`file` の出典なら、その行に `sha256:…` が無いとき。法令を箇所無しで `@法` とだけ引用したとき、箇所の書き方や、引用・宣言の形が読めないときも同じです（隣に置いたファイルは `@郵便` と丸ごと引用できます）。文書から引けるのは表なので、引用箇所は `表3`（文書の中で三つめの表）か `table3` と書いてください。それ以外の書き方は読めません。語として読めない箇所は `\"` で囲んでください（`@osha \"§1910.157\"`）。ハッシュが無いと、取り直したコピーの中身が変わっていても、check は何も言えません。",
                "A fragment cited with `@source fragment` has no `  fragment sha256:…` pin line under its `source` line; for a `file` source, the line carries no `sha256:…`. A law cited with no article (`@source` alone), and a fragment name, a citation or a `source` line whose shape cannot be read, are reported the same way (a file beside the rule may be cited whole, `@postal`). A document's fragments are its tables, so `table3` (the third table in document order; `表3` is its Japanese spelling) is the only name read. A fragment the language cannot read as one word is quoted (`@osha \"§1910.157\"`). Without a pin, a revised copy passes check in silence."
            ),
            tr!(
                "原文を読んで、転記した行が正しいことを確かめたら、`fix.text` の行を貼るか `rulec source pin <file.rule>` を実行して、いまのコピーのハッシュを書き込んでください。",
                "Once the transcribed rows are checked against the document, paste the `fix.text` line or run `rulec source pin <file.rule>` to pin the copy's digest."
            ),
            X_E037,
            &["E038", "E039", "W119"],
        )
        .with_files(ARTICLE_1),
        err(
            "E038",
            tr!("引用した箇所が変わっています", "A source fragment has changed"),
            tr!(
                "規則に書いてあるハッシュと、規則の隣にあるコピーのハッシュが違うとき。コピーを取り直した PR で出ます。注に、その箇所を引用している表・節・行が出るので、読み直すのはそこだけで済みます。",
                "The pinned digest differs from the digest of the copy beside the rule. It fails in the pull request that refreshed the copy, and names the tables, clauses and rows that cite the fragment, which is all there is to reread."
            ),
            tr!(
                "コピーの差分を読み、転記した行がまだ正しければ `fix.text` のとおりハッシュの行を書き換えてください（`rulec source pin` でも書けます）。行が変わるなら、先に行を直してください。",
                "Read the copy's diff; if the transcribed rows still hold, rewrite the pin line as `fix.text` says (`rulec source pin` writes it too). If the rows have to change, change them first."
            ),
            X_E038,
            &["E037", "E039"],
        )
        .with_files(ARTICLE_1),
        err(
            "E039",
            tr!("出典のコピーがありません", "There is no copy of a source"),
            tr!(
                "引用した箇所のコピーが規則の隣に無いとき（法令なら `sources/law/<法令ID>@<日付>/<要素>.xml`、文書の表なら文書の隣の `料金表.md.fragments/表3.tsv`）、または `file` の出典そのものが読めないとき。check は通信をせず、文書の中身も見ないので、コピーが無ければ照合できません。",
                "The copy of a cited fragment is not beside the rule — `sources/law/<law id>@<date>/<element>.xml` for a law, `tariff.md.fragments/table3.tsv` beside the document for a document's table — or the `file` source itself cannot be read. check reads neither the network nor the document, so without a copy there is nothing to compare."
            ),
            tr!(
                "`rulec source fetch <file.rule>` を実行してください。政府の法令データベース（e-Gov 法令検索）から引用箇所を取り、文書からは引いた表を取り出して、規則の隣に保存します。コピーは git に入れてください。",
                "`rulec source fetch <file.rule>` fetches the fragment from e-Gov, the Japanese government's statute database, and takes the cited tables out of a document, into the copies beside the rule. Commit the copies."
            ),
            X_E039,
            &["E037", "E038"],
        ),
        err(
            "E040",
            tr!("固定した元のファイルが、書いてあるハッシュと違います", "A file relied on differs from its pin"),
            tr!(
                "`apply` の見出しに固定が無いとき、または書いてある固定と、いま隣にある元の規則が違うとき。固定は `meaning:…`（元の規則の行と宣言から、コメント、`description`、出典の引用を除いたもの）で、前に書いた `sha256:…`（ファイルのバイト列）もそのまま読みます。元の規則の行が改正されれば、それを準用するこの規則の答えも変わっています。寄与のファイルの `to` と `after` の `signature:…` が無いとき、合わないときも同じです。こちらは足す先の形（名前、入力と出力、開いた値と開いた出力、開いた列挙と塊、寄与が読む名前の宣言）で、元の行やコメントを直しても落ちず、形が変わると、何が変わったかを言います（リポジトリの履歴に固定したときの版があれば）。固定を書いておかないと、その変化を誰も承認しないまま通ってしまいます。",
                "An `apply` heading carries no pin, or the pin differs from the callee beside the rule. The pin is `meaning:…` — the callee's rows and declarations with its comments, `description` and citations left out — and a `sha256:…` written before (the file's bytes) is still read. When the callee's rows are amended, the answers of the rule that applies it change too. The same holds for the `signature:…` on a contribution's `to` and `after` lines, missing or not matching: it is the shape of the file added to (its name, inputs and outputs, open values and outputs, open enums and blocks, and the declarations of the names the contribution reads), so a row or a comment of that file does not move it, and when the shape moves the diagnostic says what moved (when the repository's history holds the version it was pinned at). Without a pin, the change passes with nobody approving it."
            ),
            tr!(
                "`rulec diff <古い版> <新しい版>` でこの規則の答えが何件いくら動くかを見て、それでよければ、見出しを `fix.text` のとおり書き換えるか `rulec source pin <file.rule>` を実行して固定を書き直してください。寄与なら、寄与を新しい形で読み直してから、`rulec source pin <寄与のファイル>` で `signature:` を書き直してください。",
                "See with `rulec diff <old> <new>` how many answers of this rule move and by how much; once the movement is approved, rewrite the heading as `fix.text` says or run `rulec source pin <file.rule>` to pin the callee again. For a contribution, read it against the new shape, then rewrite the `signature:` with `rulec source pin <contribution>`."
            ),
            X_E040,
            &["E037", "E038", "E044", "E067"],
        )
        .with_files(CALLEE)
        .english_with_files(X_E040_EN, CALLEE_EN)
        .also_with(tr!("寄与の `signature:` が足す先の形と違う", "A contribution's `signature:` differs from the shape it adds to"), X_E040_SHAPE, E033_BASE),
        err(
            "E041",
            tr!("準用の読み替えが元の規則と合いません", "The bindings of an apply do not match the callee"),
            tr!(
                "元の規則の入力に、読み替えの無いものか、二度読み替えたものがあるとき、元の規則に無い入力や出力を書いたとき、元の規則の出力に付けた名前がこの規則に既にあるとき、一つの出力に名前を二度付けたとき、`apply` の下の行の形が読めないとき。読み替えは元の規則の入力を一つ残らず書くものなので、一つでも足りなければ、読み替えを書いていないのと同じです。",
                "A callee input is left unbound or bound twice, a binding or an output line names something the callee does not have, the name given to a callee output is already declared in this rule, one callee output is given two names, or the `apply` block is not shaped as one. Substitution is written by binding every input explicitly, so a missing binding is a substitution left unwritten."
            ),
            tr!(
                "元の規則の入力を一つずつ `<元の規則の入力> = <この規則の値>` で読み替え、出力は `<元の規則の出力> -> <名前>` で名前を付けてください。元の規則の入力と出力の名前は、診断の注に並びます。",
                "Bind each callee input with one `<callee input> = <value>` line, and rename an output with `<callee output> -> <name>`. The callee's input and output names are listed in the message."
            ),
            X_E041,
            &["E040", "E042", "E043"],
        )
        .with_files(CALLEE)
        .english_with_files(X_E041_EN, CALLEE_EN),
        err(
            "E042",
            tr!("読み替えの型が合いません", "A binding does not agree in type"),
            tr!(
                "読み替えた値の型が、元の規則の入力の型と違うとき（単位・税区分・列挙と数）。列挙どうしでは、この規則の列挙の値にあたる元の規則の値が無いとき（同じ綴りの値はそのまま対応します）、`with` にどちらにも無い値を書いたとき、リテラルが元の規則の型の値でないときも同じです。`with none -> <値>` は、この規則の値に値の無いことがあり、元の規則の入力が値の無いことを受け付けないときだけ書けます。ほかで書くと、このエラーです。",
                "The type of a bound value differs from the callee input's (unit, tax kind, an enum against a number). Between two enums: a value of this rule's enum stands for no value of the callee's (values spelled the same on both sides map by themselves), a `with` names a value neither side has, or a literal is not a value of the callee's type. `with none -> <value>` is written only where this rule's value may be absent and the callee's input takes no absent value; written anywhere else, it is this code."
            ),
            tr!(
                "型を合わせてください。列挙は `<入力> = <値> with <この規則の値> -> <元の規則の値>, …` の形で、この規則の列挙の値を全部対応づけてください。法令の「『退職』とあるのは『任期の終了』と読み替える」を、そのまま書く形です。`with none` は、値の無いことがあるときだけ書いてください。元の規則が値の無いことを受け付けるなら、`with` から外して、そのまま渡してください。",
                "Make the types agree. For enums, `<input> = <value> with <this rule's value> -> <callee's value>, …` maps every value of this rule's enum; that is the shape of \"'retirement' is read as 'end of term'\". Write `with none` only where the value may be absent; where the callee takes the absent value, leave it out of `with` and pass it on as it is."
            ),
            X_E042,
            &["E041", "E043"],
        )
        .with_files(ENUM_CALLEE)
        .english_with_files(X_E042_EN, ENUM_CALLEE_EN),
        err(
            "E043",
            tr!("渡す値が元の規則の範囲か制約に収まりません", "A value passed leaves the callee's range or constraint"),
            tr!(
                "読み替えた値の取りうる範囲が、元の規則の入力の `range` からはみ出すとき（注に、はみ出す値の例が一つ出ます）、または元の規則の `constraint` がこの規則の宣言から導けないとき。順のある列挙の入力の `range`（`range >=basic <=gold`）では、渡す値が取りうる列挙の値を `with` で元の規則の値に読み替えて、範囲に入らない値があるときです。取りうる値は、入力ならその範囲（範囲が無ければ列挙の全部）、リテラルならその値、表の出力なら行が書く値（名前を書く行は、その名前が取りうる値）、二つの値の `max` と `min` なら、二つが取りうる値のうち答えになりうるもの（`max` は二つの一番前の値の後のほうより前にならず、`min` は二つの一番後の値の前のほうより後にならない）です。`fold` の答えなどほかの式で決まる値は、check がその値を求めないので列挙の全部とし、範囲の外の値があれば、渡す値の取りうる値が分からないと言います。元の規則の完全性はその範囲と制約の上でしか証明されていないので、外の値には定義がありません。値の無いことがある値（`T?` の入力や要素のフィールド、`none` を書く行のある表の出力、それらを指すだけの定義）を、`?` の無い元の規則の入力やフィールドに渡すときにも出ます。元の規則の表には、値が無いときの答えがありません。`with none -> <値>` で値の無いことを元の規則の値に読み替えていれば出ません。`fix.text` は付けません。元の規則に行を足すのは、別の承認の話だからです。",
                "The interval of a bound value reaches outside the callee input's `range` (the point outside is shown as the witness), or a `constraint` of the callee does not follow from this rule's declarations. For the `range` of an input of an ordered enum (`range >=basic <=gold`), it is a value the bound value can take, read as the callee's through `with`, that the range does not let in. What a value can take is an input's range (every value of its enum without one), a literal's value, what the rows of a table write for its output (what a name can take, for a row that writes one), and, for a `max` or a `min` of two values, what the two can take that the answer can be (a `max` comes to nothing before the later of their first values, a `min` to nothing after the earlier of their last ones). A value a fold or another expression decides is taken to be any value of its enum, since check does not work out which ones those come to, and what is passed is then said to be not known. The callee's completeness was proved over that range and constraint; outside them there is no definition. It is also a value that may be absent — an input or a field of an element declared `T?`, the output of a table a row of which answers `none`, a definition that only names one — passed to an input or a field of the callee that is not optional: the callee's tables have no answer for the absent value. Where `with none -> <value>` reads it as a value of the callee's, there is nothing to say. There is no `fix.text`: adding a row to the callee belongs to another unit of approval."
            ),
            tr!(
                "この規則の入力の範囲を元の規則の範囲まで狭めるか、はみ出す部分をこの規則の節で定めてください。どちらにするかは業務の判断です。列挙なら、この規則の入力に同じ `range` を書くか（順のある列挙のとき）、範囲の中の値だけを書く表で値を決めて渡すか、`with` で範囲の中の値に読み替えてください。制約なら、同じ関係を `constraint` で宣言してください。値の無いことなら、`with none -> <値>` で元の規則の値に読み替えるか（列挙のとき）、値がいつもあるならこの規則の名前を `?` の無い型にするか、`none` にも答える表をこの規則に書いて値を決めて渡してください。数や要素のフィールドで値の無いことがあるなら、元の規則が値の無いことを受け付ける変更になります。",
                "Narrow this rule's input range to the callee's, or define the region outside it in a clause of this rule; which is a business decision. For an enum, declare the same `range` on this rule's input (of an ordered enum), decide the value passed in a table that writes only values inside the range, or read the values outside it as values inside it with `with`. For a constraint, declare the same relation with `constraint`. For the absent value, read it as a value of the callee's with `with none -> <value>` (an enum), give this rule's name a type that is not optional where it always has a value, or decide the value in a table of this rule that answers `none` too and pass that. A number or a field of an element that can be absent needs the callee to take the absent value, which is a change to the callee."
            ),
            X_E043,
            &["E041", "E042", "E101"],
        )
        .with_files(CALLEE)
        .english_with_files(X_E043_EN, CALLEE_EN)
        .also_with(tr!("順のある列挙の入力の範囲", "The range of an input of an ordered enum"), X_E043_ENUM, TIER_CALLEE)
        .also_with(tr!("値の無いことがある値", "A value that may be absent"), X_E043_ABSENT, ABSENT_CALLEE),
        err(
            "E044",
            tr!("その規則は準用できません", "That rule cannot be applied"),
            tr!(
                "元の規則が読めないとき、元の規則が `check` を通らないとき（注に、出たコードが並びます）、準用が輪になっているとき（ある規則を準用する規則を、その規則が、直接かほかの規則を通して準用している。輪の全部のファイルを挙げます）。通らない規則を準用しても、通らない規則になるだけで、自分を通して自分を読む規則は答えが決まりません。準用している規則の準用、並びをたどる規則の準用、元の規則の列挙を出す表の準用はできます。元の規則の出力が、値の無いことのある数（`money[円]?` など）のときにも出ます。準用した出力は、元の規則の丸めを掛けた定義になり、値の無いことのある値には丸めを掛けられないからです。値の無いことのある列挙・真偽・日付・文字列の出力は準用でき、値の無いことをそのまま渡します。",
                "The callee cannot be read, does not pass `check` (the codes are listed), or the applies go round: a rule applies a rule that applies it back, directly or through others (every file of the ring is listed). A broken rule expanded is a broken rule, and a rule that reads itself through itself has no answer. A rule that applies another, one that walks a sequence, and one whose tables produce its own enums can all be applied. It is also an output of the callee that is a number that may have no value (`money[JPY]?`): an applied output becomes a definition rounded as the callee declares, and a value that may be absent cannot be rounded. An optional enum, truth value, date or string can be applied, and hands the absent value on."
            ),
            tr!(
                "先に元の規則を直してください。輪なら、どこか一つの `apply` を外してください。",
                "Fix the callee first. For a ring, take one of the `apply` lines out."
            ),
            X_E044,
            &["E040", "E041"],
        )
        .with_files(BROKEN_CALLEE)
        .english_with_files(X_E044_EN, BROKEN_CALLEE_EN)
        .also_with(tr!("準用の輪", "Applies that go round"), X_E044_RING, E044_OTHER),
        err(
            "E045",
            tr!("出力を共有する表に、出力の列が二つ以上あります", "A table that shares an output has two or more output columns"),
            tr!(
                "ある出力を二つ以上の表が定めていて（または `overrides` で結ばれていて）、そのうちの表に出力の列が二つ以上あるとき。行が二つの出力の定義を束ねていると、一方だけが上書きされたときにもう一方の値の出どころが決まらず、生成コードでは同じ行の条件を二度書くことになります。",
                "An output is defined by two or more tables (or tables are joined by `overrides`), and one of them has two or more output columns. A row that bundles two definitions leaves the other value's origin undecided when only one is overridden, and the generated code would have to write the row's condition twice."
            ),
            tr!("二つ目の出力を、別の表に分けてください。", "Move the second output to a table of its own."),
            X_E045,
            &["E036", "E105"],
        )
        .english(X_E045_EN),
        err(
            "E046",
            tr!("`clause` の形が読めません", "A `clause` is not shaped like this"),
            tr!(
                "`clause` の見出しに名前か `->` か出力が無いとき、`when` か `then` の行が無いか二度あるとき、`when` の条件が `<列> <セル> and …` の形でないとき（列が無い、同じ列が二度ある、条件が無い）。節は一行の表なので、条件と値が一つずつ要ります。",
                "The `clause` heading lacks a name, the `->` or the output; the `when` or `then` line is missing or appears twice; or the `when` condition is not `<column> <cell> and …` (a part without a column, a column twice, a column without a condition). A clause is a one-row table, so it needs exactly one condition and one value."
            ),
            tr!(
                "`clause <名前>(<別名>) -> <出力>` の下に `when <列> <セル> and …`（条件が無ければ `when always`）と `then <値>` を一行ずつ書いてください。優先する相手があれば `overrides <相手>` を足してください。",
                "Under `clause <name>(<alias>) -> <output>`, write one `when <column> <cell> and …` line (`when always` when there is no condition) and one `then <value>` line. Add `overrides <target>` when it takes precedence over something."
            ),
            X_E046,
            &["E008", "E035", "E045"],
        )
        .english(X_E046_EN),
        err(
            "E047",
            tr!("宣言の後ろに余分な語があります", "Extra token after the declaration"),
            tr!(
                "宣言の行に、その行が読まない語が残っているとき。行ごとに読む語は決まっていて、入力の行は `range`・`contract_only`・`random`、要素のフィールドの行は `range` と `contract_only`、出力の行は `range` と `round`、`derive` の行と `count`・`sum`・`max`・`min` の行は `range` だけです。`shape` の行と、入力の `from` のパスの後ろ（`where`・`empty` のほか）には何も置けません。rulec のパーサーは行から欲しい語を探して残りを読み飛ばすので、こうした語はいままで黙って捨てられていました。範囲の後ろに書いた税区分（`range >=0円 <=10000円 incl_tax`）も、単位が単位として読めずに余った分も、`derive` の `round` も、`from` の後ろの `range` も、どれも通っていました。範囲は完全性の検査が調べる値の全体で、生成コードの入口のガードでもあるので、境界が一つ落ちると、片側が欠けた範囲のまま「完全」と答えることになります。",
                "A declaration line holds a word the line does not read. Each kind of line reads its own words: an input's `range`, `contract_only` and `random`; a field of an element's `range` and `contract_only`; an output's `range` and `round`; a `derive` and a `count`, `sum`, `max` or `min` line `range` alone. A `shape` line holds nothing after its declaration, and an input's `from` nothing after its path but `where` and `empty`. The readers look along the line for the word they want and step over everything else, so such a word used to be dropped in silence: a tax flag written after the range, as in `range >=0JPY <=10_000JPY incl_tax`, what is left of a bound whose unit did not lex as one, a `round` on a `derive`, a `range` after a `from`. A range is the universe the completeness proof quantifies over and the entry guard of the generated code, so a bound lost this way is answered \"complete\" with one side missing."
            ),
            tr!(
                "その語を消すか、宣言の一部として正しい形に直してください。税区分や刻みは型の括弧の中に（`money[円, incl_tax]`、`rate[step 0.1%]`）、範囲は `range >=<値> <=<値>`、丸めは `round <向き>(<刻み>)` の形で書いてください。入力の範囲と印は `from` の前に書きます。導出の値を丸めるなら、式の中で `down(<式>, 1円)` のように丸めます。",
                "Remove the word, or write it in the form the declaration takes. A tax flag or a step goes inside the type's brackets (`money[JPY, incl_tax]`, `rate[step 0.1%]`); a range is `range >=<value> <=<value>`; a rounding is `round <mode>(<grid>)`. An input's range and marks go before its `from`; a derived value is rounded inside its expression, as `down(<expression>, 1JPY)`."
            ),
            X_E047,
            &["E011", "E103", "E104"],
        )
        .english(X_E047_EN),
        err(
            "E049",
            tr!("桁区切りのカンマは書けません", "A thousands separator cannot be written"),
            tr!(
                "数を `1,000` や `1,949,000円` のように、1〜3 桁のあとに `,` と 3 桁の組を続けて書いたとき。`,` はセルの中で集合の要素を区切る記号なので、そのままでは二つ以上の値に読まれます。いままでは黙ってそう読んでいて、数の列の `<=1,000` は `<=1` として検査を通り、金額の列では `1` に単位が無いという的外れな E103 で止まっていました。文書の数字をそのまま転記すると、このエラーが出ます。",
                "A number is written as a document writes it, one to three digits followed by `,` and groups of three: `1,000`, `1,949,000JPY`. Inside a cell `,` separates the members of a set, so as it stands the figure reads as more than one value. It used to be read that way without a word: `<=1,000` in a column of numbers passed the check as `<=1`, and in a column of money it stopped at an E103 about `1` having no unit. Copying a document's figures as they stand lands here."
            ),
            tr!(
                "カンマを外して `1000` と書いてください。桁を区切りたいときは `1_000` と書けます。`fix.text` には、書き直したリテラルが入っています。",
                "Take the commas out and write `1000`; to group the digits, write `1_000`. `fix.text` is the literal rewritten."
            ),
            X_E049,
            &["E014", "E103"],
        )
        .english(X_E049_EN),
        err(
            "E048",
            tr!("この型には足し算も掛け算もありません", "This type has no arithmetic"),
            tr!(
                "順序はあるが演算の無い型を、`+ - × ÷` のどれかに使ったとき。`date`、`temperature[℃]`・`temperature[℉]`、`sound[dB]` の三つがそれです。℃ は 0 が「無い」を意味しない目盛りなので `気温 × 2` に意味が無く、dB は対数なので二つ足しても音が二つ分にはならず、日付は暦日なので引き算の結果を入れる型がありません。どれも規則の中では閾値としてしか現れないので、比較と `range` だけを残して演算を落としています。",
                "A type that is ordered but has no arithmetic is used with `+ - × ÷`. There are three: `date`, `temperature[℃]`/`temperature[℉]`, and `sound[dB]`. A ℃ has a displaced zero, so `temp × 2` means nothing; a decibel is a logarithm, so adding two of them is not two sounds' worth; and a date is a calendar day, with no type to hold the result of subtracting one. All three appear in rules only as thresholds, so comparison and `range` are kept and the arithmetic is dropped."
            ),
            tr!(
                "閾値として比べるか、`range` に書いてください。差や倍率そのものが業務ルールなら、計算した結果を入力として受け取るか、表で引いてください。二つの日付のあいだの日数は、呼び出し側で数えて `number` か `duration` として渡してください。",
                "Compare it against a threshold, or write it in a `range`. Where a difference or a multiple is itself the rule, take the computed value as an input, or look it up in a table. The days between two dates are counted on the calling side and passed in as a `number` or a `duration`."
            ),
            X_E048,
            &["E103", "E112", "E115"],
        )
        .english(X_E048_EN),
        err(
            "E050",
            tr!("`machine` の節の形が違います", "The `machine` section is not shaped right"),
            tr!(
                "`machine` の見出しに名前か `over <表>` が無いとき、`carry` か `initial` の行が無いとき、状態の `initial` の行が二行あるとき、`machine` の中に `carry`・`held`・`initial`・`final`・`never`・`once`・`always` のほかの行があるとき、規則に `machine` が二つあるとき。",
                "The `machine` heading lacks a name or `over <table>`; the `carry` or `initial` line is missing, or the state's `initial` line is written twice; a line under `machine` is not `carry`, `held`, `initial`, `final`, `never`, `once` or `always`; or the rule has two `machine` sections."
            ),
            tr!(
                "形は `machine <名前>(<ascii>) over <表>` と、その下の `carry <入力> -> <出力>` と `initial <状態>`（どちらも必須）、`final <状態>, …`、`never <状態>, … after <状態>, …`、`once <出力> <セル>`（どれも任意）です。ほかの値も一緒に持ち越すなら、`carry` の行と `initial <入力> <値>` の行を足してください（E072）。",
                "The shape is `machine <name>(<ascii>) over <table>`, then `carry <input> -> <output>` and `initial <state>` (both required), and `final <state>, …`, `never <state>, … after <state>, …` and `once <output> <cell>` (all optional). To carry more values along, add a `carry` line and an `initial <input> <value>` line for each (E072)."
            ),
            X_E050,
            &["E051", "E052", "E053"],
        ),
        err(
            "E051",
            tr!("`carry` か `over` が宣言と噛み合いません", "`carry` or `over` does not fit the declarations"),
            tr!(
                "`carry` の左が入力でないとき、右が出力でないとき、最初の `carry` の二つが同じ列挙でないとき、二行目からの `carry` の二つが同じ型でないとき、`over` の表が無いとき、その表が持ち越す出力を決めていないとき。状態が有限個の値の列挙なので、呼び出しの並びについての主張を確かめられます。",
                "The left of `carry` is not an input, the right is not an output, the two of the first `carry` line are not of one enum, the two of a later `carry` line are not of one type, the `over` table does not exist, or it does not decide the carried output. It is because the state is an enum of finitely many values that the claims about sequences of calls are decidable."
            ),
            tr!(
                "状態の入力と状態の出力を同じ列挙で宣言して `carry <入力> -> <出力>` と書き、`over` には、持ち越す出力を出力の列に持つ表の名前を書いてください。",
                "Declare the state as an input and an output of one enum, write `carry <input> -> <output>`, and name after `over` the table that has the carried output as an output column."
            ),
            X_E051,
            &["E050", "E052"],
        ),
        err(
            "E052",
            tr!("ステートマシンに書いた状態が噛み合いません", "A state the machine names does not fit"),
            tr!(
                "`initial`・`final`・`never` の行に、持ち越す状態の列挙に無い値があるとき。または `never` の両側に同じ状態があるとき。その主張は、その状態に留まる最初の呼び出しで破れるので、意味のある主張になりません。",
                "An `initial`, `final` or `never` line names a value the enum of the carried state does not have, or `never` names one state on both sides — which the first call that stays in it breaks, so it cannot be what anyone means."
            ),
            tr!(
                "列挙の値の綴りで書いてください。`never` の両側には、別の状態を書いてください。",
                "Spell the value as the enum does. Write different states on the two sides of `never`."
            ),
            X_E052,
            &["E050", "E126"],
        ),
        err(
            "E053",
            tr!("`once` の行が宣言と噛み合いません", "The `once` line does not fit the declarations"),
            tr!(
                "`once` の出力が宣言されていないとき、持ち越す状態か持ち越すほかの値の出力であるとき、セルが `-` のとき。持ち越す値は、それを保つ呼び出しのたびに同じ値を返すので、`once` で数えると、保つだけで主張が破れます。セルは出力の型に照らして、表のセルと同じ検査を受けます（E103 など）。",
                "The output of `once` is not declared, is the carried state or another carried value, or the cell is `-`. A carried value is answered again on every call that keeps it, so counting it with `once` breaks on the first call that does. The cell is held to the output's type exactly as a table's cell is (E103 and the rest)."
            ),
            tr!(
                "`once <出力> <セル>` の出力には、持ち越す状態ではない出力を書いてください（`once 返金額 >0円`）。状態について言うなら `never … after …` を使ってください。",
                "Name an output other than the carried state (`once refund >0JPY`). Say it about states with `never … after …`."
            ),
            X_E053,
            &["E127", "E103"],
        ),
        err(
            "E054",
            tr!("並びを畳む規則は、ステートマシンの一歩になれません", "A rule that folds a sequence cannot be the step of a machine"),
            tr!(
                "`fold` のある規則に `machine` を書いたとき。答えが並び全体で決まるので、入力を決まった数の列の区画に分けられず、一回の呼び出しの行き先を数え上げられません。`diff` が同じ規則を受け付けないのと同じ理由です。",
                "A rule with a `fold` has a `machine`. Its answer depends on the whole sequence, so the inputs do not cut into finitely many columns and the transitions of one call cannot be counted — the reason `diff` refuses the same rule."
            ),
            tr!(
                "並びは呼び出す側で読み、見つけたもの（件数や合計）を入力として渡してください。`count` と `sum` で数える規則は、ステートマシンの一歩になれます。",
                "Read the sequence on the calling side and pass what it found (a count, a total) as an input. A rule that counts with `count` or `sum` can be the step of a machine."
            ),
            X_E054,
            &["E050", "E028"],
        )
        .english(X_E054_EN),
        err(
            "E055",
            tr!("`scenario` の形が違います", "A `scenario` is not shaped right"),
            tr!(
                "`machine` の無い規則に `scenario` を書いたとき、名前が無いか二つが同じ名前のとき、行が無いとき、見出しに同じ列が二つあるとき、持ち越す入力（状態とほかの持ち越す値）の列があるとき、ほかの入力の列が足りないとき。一行目の呼び出しは `initial` の値から、二行目からは一つ前の呼び出しが返した値から始まるので、持ち越す入力には列がありません。",
                "A `scenario` is written in a rule without a `machine`; it has no name, or two share one; it has no rows; its heading has one column twice; it has a column for a carried input (the state, or another carried value); or it lacks a column for another input. The first call starts from the `initial` values and every later one from what the call before it answered, so a carried input has no column."
            ),
            tr!(
                "`scenario <名前>(<ascii>)` の直下に表を書いてください。見出しは、持ち越す入力のほかのすべての入力、`->`、すべての出力です。持ち越す状態が無いなら、一行ずつの `examples` で書けます。",
                "Write the table right under `scenario <name>(<ascii>)`. The header is every input but the carried one, then `->`, then every output. With no state carried from one call to the next, write the rows as `examples`."
            ),
            X_E055,
            &["E107", "E111", "E050"],
        ),
        err(
            "E056",
            tr!("`held` の行が宣言と噛み合いません", "The `held` line does not fit the declarations"),
            tr!(
                "`held` に、入力でない名前か、持ち越す入力（状態とほかの持ち越す値）か、同じ名前を二度書いたとき。`held` は、一つの案件が最初の呼び出しから最後の呼び出しまで同じ値で渡す入力を言います（注文の金額、申し込んだ人の区分）。書いておくと、検査は、そういう入力を呼び出しごとに変えた並びを、主張が破れる例として挙げなくなります。",
                "`held` names something that is not an input, a carried input (the state, or another carried value), or one name twice. `held` says which inputs one case passes with the same value on every call, from its first to its last — the amount of an order, the class of the person who applied — and the check then stops offering sequences of calls that change one of them as counterexamples."
            ),
            tr!(
                "入力の名前を書いてください。持ち越す入力は呼び出しのたびに一つ前の答えで入れ替わるので、`held` にはなりません。",
                "Name inputs. The carried input is replaced by the answer of the call before on every call, so it cannot be `held`."
            ),
            X_E056,
            &["E050", "E051", "W126"],
        ),
        err(
            "E057",
            tr!("宣言に型がありません", "The declaration has no type"),
            tr!(
                "`inputs`・`outputs`・`elements` の行、または `define`・`derive` の行に、名前だけがあって型が無いとき。この行は、いままで黙って捨てられていました。規則は書いた人の思うより入力が一つ少ないまま検査を通り、例やベクタがその名前を渡すと、知らない名前としてエラーになっていました。",
                "A line under `inputs`, `outputs` or `elements`, or a `define` or `derive` line, has a name and no type. Such a line used to be dropped in silence: the rule passed the check with one input fewer than its author wrote, and an example or a record that named it was then refused for a name nobody declared."
            ),
            tr!(
                "`<名前>(<別名>) : <型>` の形で型を書いてください（`: money[円, incl_tax]`、`: 都道府県`、`: bool`）。数の型なら `range` も要ります。",
                "Write the type as `<name>(<alias>) : <type>` (`: money[JPY, incl_tax]`, `: <enum>`, `: bool`). A numeric type needs a `range` too."
            ),
            X_E057,
            &["E011", "E047", "E012"],
        ),
        err(
            "E058",
            tr!("宣言の行を読めません", "A declaration line cannot be read"),
            tr!(
                "`define`・`derive`・`result`・`enum`・`group`・`table` のような宣言の語で始まる行を、その宣言として読めないとき。名前が無い、`=` の前に余分な語がある、などです。こういう行は、いままで何も言われずに捨てられていました。`derive` の行は捨てられもせず、検査が終わらなくなっていました。",
                "A line that starts with a declaring word — `define`, `derive`, `result`, `enum`, `group`, `table` and the like — cannot be read as that declaration: it has no name, something stands before its `=`, and so on. Such a line used to be dropped with nothing said, and a `derive` line was not even dropped: the check never finished."
            ),
            tr!(
                "診断の注記にある形に書き直してください（`define 大口(bulk) : bool = 注文金額 >= 3万円`）。どの宣言の形も docs/reference.md にあります。",
                "Rewrite the line in the shape the note gives (`define bulk(bulk) : bool = order_total >= 30_000JPY`). Every declaration's shape is in docs/reference.md."
            ),
            X_E058,
            &["E006", "E057", "E059", "E005"],
        ),
        err(
            "E059",
            tr!("式を読めません", "The expression cannot be read"),
            tr!(
                "`=` などの右に書いた式を、最後の語まで読めないとき。演算子の右が空（`x +`）、`(` が閉じていない、値が二つ演算子なしで並んでいる（`金額 税`）、符号が数にくっついている（`金額 -100円`）、などです。いままでは読めたところまでを式にして、残りを黙って捨てていました。`金額 税` は `金額` と読まれて検査を通り、`x +` は宣言ごと消えていました。",
                "An expression — on the right of `=`, of `->` in a `fold`, and so on — cannot be read to its last token: nothing on the right of an operator (`x +`), a `(` never closed, two values with no operator between them (`amount tax`), a sign stuck to a number (`amount -100JPY`). What could be read used to become the expression and the rest was dropped: `amount tax` passed the check as `amount`, and `x +` took its whole declaration with it."
            ),
            tr!(
                "抜けている演算子や括弧を補ってください。引くときは `金額 - 100円` のように、記号の後ろを空けてください。記号と数字がくっついていると、負の数として読まれます。",
                "Supply the missing operator or parenthesis. To subtract, leave a space after the sign, as in `amount - 100JPY`: a sign touching the digits makes a negative number."
            ),
            X_E059,
            &["E058", "E118", "E115"],
        )
        .english(X_E059_EN),
        err(
            "E060",
            tr!("刻みが正の値ではありません", "A step is not positive"),
            tr!(
                "型の刻み（`rate[step 0%]`）、出力の丸めの刻み（`round up(0円)`）、式の中の丸めの刻み（`down(x, 0円)`）が 0 か負の数のとき。刻み 0 の丸めは検査を通り、生成コードの中で 0 で割って、実行時に止まっていました（Python なら ZeroDivisionError）。刻み 0 の型は、黙って刻み 1 として読まれていました。",
                "The step of a type (`rate[step 0%]`), the rounding grid of an output (`round up(0JPY)`) or of a rounding call (`down(x, 0JPY)`) is zero or negative. A grid of zero passed the check and the generated code divided by it at run time (Python stopped with ZeroDivisionError); a step of zero was quietly read as a step of one."
            ),
            tr!(
                "0 より大きい刻みを書いてください（`round up(1円)`、`rate[step 0.1%]`）。",
                "Write a step greater than zero (`round up(1JPY)`, `rate[step 0.1%]`)."
            ),
            X_E060,
            &["E104", "E114", "E106"],
        )
        .english(X_E060_EN),
        err(
            "E061",
            tr!("範囲が空です", "The range is empty"),
            tr!(
                "`range` に入る値が一つも無いとき。下の端が上の端を超えている（`range >=10円 <=0円`）か、両端が同じ値で片方が `>` か `<` のときです。こういう範囲は、いままで何も言われずに通っていました。完全性の証明は何も無い集合について「漏れなし」と答え、生成コードの入口はどの呼び出しも受け付けていませんでした。",
                "No value lies in a `range`: the lower end is past the upper one (`range >=10JPY <=0JPY`), or the two are equal and one of them is `>` or `<`. Such a range used to pass with nothing said: the completeness proof answered \"complete\" over nothing, and the generated code's entry refused every call."
            ),
            tr!(
                "書き間違えた端を直してください（`range >=0円 <=10円`）。",
                "Correct the end that was mistyped (`range >=0JPY <=10JPY`)."
            ),
            X_E061,
            &["E112", "E103", "E060"],
        )
        .english(X_E061_EN),
        err(
            "E062",
            tr!("ありえない日付です", "There is no such date"),
            tr!(
                "日付の形（`2026-04-01`）をしているのに、月が 1〜12 の外か、日がその月の日数を超えるとき（`2026-02-30`、`2026-01-99`）。こういう日付は、いままで形だけを見て読まれていました。検査器は日の通し番号として数え（1 月 99 日は 4 月 9 日）、生成した Python は実行時に止まり、JavaScript の `Date` は黙って次の月へ繰り越すので、言語ごとに答えが割れていました。",
                "A literal has the shape of a date (`2026-04-01`) but no such day: the month is outside 1 to 12, or the day is past the end of its month (`2026-02-30`, `2026-01-99`). Such a date used to be read by its shape alone: the checker counted on from the first of the month (January 99th was April 9th), the generated Python stopped at run time, and JavaScript's `Date` would have rolled it over quietly, so the languages disagreed."
            ),
            tr!(
                "ある日付に直してください。月末を言いたいなら、その月の最後の日を書いてください（`2026-02-28`）。",
                "Write a day that exists. For the end of a month, write its last day (`2026-02-28`)."
            ),
            X_E062,
            &["E002", "E103"],
        ),
        err(
            "E063",
            tr!("セルを読めません", "The cell cannot be read"),
            tr!(
                "表・例・並びのセルに、セルとして読めない語があるとき。比較の後ろの余分な語（`<=0円 + false`）、値の後ろの語（`false 0円`）、値でない語（`+`）、見出しの列に名前が無い、などです。いままでは読めたところまでをセルにして、残りを捨てていました。読めないセルは丸ごと捨てていたので、後ろのセルが一つずつ左の列へずれていました。",
                "A cell of a table, of the examples or of a sequence holds something a cell cannot: words after a comparison (`<=0JPY + false`), a word after a value (`false 0JPY`), a word that is no value (`+`), a header column with no name. What could be read used to become the cell and the rest was dropped, and a cell that could not be read at all was dropped whole, so every cell after it moved one column to the left."
            ),
            tr!(
                "セルに書けるのは、値一つ、`-`、`none`、比較、カンマで区切った値の集合、`not:` と集合、`starts_with` と文字列です。一つの列に二つの条件を書くなら、比較を並べてください（`>=1 <10`）。",
                "A cell holds one value, `-`, `none`, a comparison, a set of values separated by commas, `not:` with a set, or `starts_with` with a string. Two conditions on one column are two comparisons side by side (`>=1 <10`)."
            ),
            X_E063,
            &["E008", "E010", "E014", "E064"],
        )
        .english(X_E063_EN),
        err(
            "E064",
            tr!("行が見出しと合いません", "The row does not fit the header"),
            tr!(
                "表・並び・手順の例・例の行のセルの数が、見出しの列の数と違うとき。または、最後の `|` の後ろに語があるとき（行を閉じる `|` の書き忘れ）。いままでは、閉じていない行の最後のセルが黙って捨てられ、セルの数の合わない行もそのまま読まれていました。参照評価器と生成コードがその行を別々に読み、答えが割れていました。例の表では、見出しに出力の列が無いか行の期待値が足りなければ E111 が、並びの列が無ければ E025 が先に出て、どの列かを言います。",
                "A row of a table, of a sequence, of a scenario or of the examples has a different number of cells from the header's columns, or words follow its last `|` — the closing bar was left off. The last cell of such a row used to be dropped with nothing said, and a row whose cells did not match was read as it stood; the reference evaluator and the generated code then read the row two different ways and answered differently. In the examples, a header without a column for an output, or a row short of an expected value, is E111, and a header without the sequence's column is E025: each names the column first."
            ),
            tr!(
                "一つの列に一つのセルを書き、行を `|` で閉じてください。どの値でもよい列には `-` を書いてください。",
                "Write one cell per column and close the row with `|`. A column that takes any value gets `-`."
            ),
            X_E064,
            &["E063", "E008", "E111"],
        ),
        err(
            "E065",
            tr!("`range from koyomi` の形か置き場所が違います", "`range from koyomi` is misshapen or misplaced"),
            tr!(
                "`range from` の後ろが `koyomi \"<ファイル>\" date <日付の名前>` の形になっていないとき、日付でない入力に書いたとき、出力・導出・並びの要素に書いたとき。koyomi の日付がとりうる日は、呼び出し側が渡す日付の範囲にだけなります。",
                "What follows `range from` is not `koyomi \"<file>\" date <name of a date>`, or it is written on an input that is not a date, or on an output, a derive or a field of an element. The days a koyomi date comes to can only be the range of a date the caller passes."
            ),
            tr!(
                "日付の入力に `range from koyomi \"<ファイル>\" date <日付の名前>` と書いてください。ファイルは、規則のディレクトリからの相対パスです。",
                "Write `range from koyomi \"<file>\" date <name of a date>` on a date input. The file is a path from the rule's directory."
            ),
            X_E065,
            &["E129", "E130"],
        ),
        err(
            "E066",
            tr!("`enums` のファイルの形と、そこから読む `import enum` の形が違います", "An `enums` file, or an `import enum` from one, is misshapen"),
            tr!(
                "見出しが `enums` のファイルに、`description`・`import`・`enum`・`group`・`prerequisites` 以外の行（表、入力、出力、定義、`import enum` など）を書いたとき。`import enum <列挙> from \"<ファイル>\"` が読むファイルが `enums` のファイルでないとき、または check を通らないとき。`enums` のファイルは規則どうしが分け合う列挙を置く場所で、何も決めず、規則の関数も生成しません。",
                "A file headed `enums` holds a line other than `description`, `import`, `enum`, `group` and `prerequisites` (a table, inputs, outputs, a definition, an `import enum`, …); or `import enum <enum> from \"<file>\"` names a file that is not an `enums` file, or one that does not pass check. An `enums` file holds the enums rules share: it decides nothing, and no rule's function is generated from it."
            ),
            tr!(
                "表や入力は、その列挙を `import enum … from` で読む規則の側に書いてください。読む先のファイルは `enums <名前> v<版>` で始め、check を通るように直してください。",
                "Write the tables and the inputs in a rule that reads the enum with `import enum … from`. Start the file it reads with `enums <name> v<version>`, and make it pass check."
            ),
            X_E066,
            &["E013", "E034"],
        )
        .english(X_E066_EN),
        err(
            "E069",
            tr!("順序つきの列挙と比べるセル、または表示名の形が違います", "An ordered enum, a comparison on one, or a display name is misshapen"),
            tr!(
                "値を `|` で区切った（順序の無い）列挙の列に `<v` や `>=v` のような比べるセルを書いたとき、そういう列挙の二つの値を `<` などで比べたとき、`max`・`min` に渡したとき、入力に範囲を書いたとき。一つの列挙の値の区切りに `<` と `|` を混ぜたとき。範囲に入る値が一つも無いとき、要素のフィールドの列挙に範囲を書いたとき、列挙の列の `max`・`min` の行に範囲を書いたとき。値の表示名が空か制御文字を含むとき、同じ列挙の二つの値の表示名が同じとき。値を `<` で区切った列挙は、書いた順が値の順序になり、比べるセルは、比べた値より前（後）の値の集合として読まれます。",
                "A cell that compares (`<v`, `>=v`) is written in a column of an enum whose values are separated by `|`, and so have no order; or two values of such an enum are compared with `<` and the like, passed to `max` or `min`, or held to a range on an input. Or one enum separates its values with both `<` and `|`. Or a range lets no value in, or an enum field of an element is given a range, or a range is written on the line of a `max` or a `min` of an enum column. Or a value's display name is empty or holds a control character, or two values of one enum have the same display name. The values of an enum separated by `<` are ordered as written, and a comparison takes the values before (after) the one it is against."
            ),
            tr!(
                "宣言の順が意味を持つなら、値を `<` で区切ってください（`enum 段階 = 一般 < シルバー < ゴールド`）。持たないなら、値か集合で書いてください。区切りは `<` か `|` のどちらかにそろえ、表示名は空でない、ほかの値と違う名前にしてください。",
                "If the order the values are declared in means something, separate them with `<` (`enum tier = basic < silver < gold`); if not, write values or sets. Use one separator throughout, and give each value a display name that is not empty and differs from the others'."
            ),
            X_E069,
            &["E012", "E103"],
        )
        .english(X_E069_EN),
        err(
            "E071",
            tr!("前提のつながりの形が違います", "The prerequisites are misshapen"),
            tr!(
                "`prerequisites` の塊で、`over` の後が列挙でないとき、列挙の値でない名前を書いたとき、一つの値を二か所に書いたとき（`start` と自分の行、二つの行、一つの行の中で二度）、`start` の行にも自分の行にも無い値があるとき、`start` の行が無いとき、行の形が読めないとき。`ordered by` が名前を挙げた表が無いとき、塊より下にあるとき、その列挙の列一つを条件にし、値ごとに順序のある列挙か `number` の値を一つ書く表でないとき、`enums` のファイルに `ordered by` を書いたとき。どの値も、何も要らない（`start`）か、要るものを書いた自分の行を持つか、手に入らない値（`requires nothing \"<理由>\"`）なので、値を足して前提を書き忘れると、ここで止まります。",
                "In a `prerequisites` block: `over` names something that is not an enum; a name is not a value of the enum; a value is written in two places (on `start` and on a line of its own, on two lines, or twice on one line); a value is neither on `start` nor on a line of its own; there is no `start` line; or a line cannot be read. Or the table `ordered by` names is not there, is declared below the block, or does not test one column of the enum and write, for each value, one value of an ordered enum or one `number`; or `ordered by` is written in an `enums` file. Every value needs nothing (`start`), has a line of its own saying what it needs, or is one no case can take (`requires nothing \"<reason>\"`), so a value added without its prerequisites stops here."
            ),
            tr!(
                "値ごとに一か所だけ書いてください。何も要らない値は `start` に、要るものがある値は `<値> requires <前提>, …` の行に、手に入らない値は `<値> requires nothing \"<理由>\"` に書きます。`ordered by` の表は塊より上に置き、列挙の列一つを条件にして、順序のある列挙か `number` の値を行ごとにそのまま書いてください。",
                "Write each value in one place: a value that needs nothing on `start`, one that needs something on a `<value> requires <prerequisite>, …` line, and one no case can take as `<value> requires nothing \"<reason>\"`. Declare the table of `ordered by` above the block, make it test the enum's column alone, and write one value of an ordered enum or one `number` in each row."
            ),
            X_E071,
            &["E136", "E137", "E138"],
        ),
        err(
            "E072",
            tr!("持ち越す値の書き方が違います", "A carried value is not written right"),
            tr!(
                "二行目からの `carry` が、範囲に両端の無い数か、日付・文字列・省略できる値を持ち越すとき、同じ値を二度持ち越すとき、持ち越す値に `initial <入力> <値>` の行が無いか二行あるとき、その値が型や範囲に合わないとき、`initial <入力> <値>` が状態や持ち越さない入力を書いたとき、`always` の行の形が違うとき、`always` が持ち越す値でない名前か状態そのものを書いたとき。持ち越す数に範囲の両端があるので、案件がとりうる値は有限個になり、呼び出しの並びについての主張を確かめられます。",
                "A `carry` line after the first carries a number whose range lacks an end, or a date, a string or an optional value; a value is carried twice; a carried value has no `initial <input> <value>` line, or two, or one whose value does not fit its type or range; an `initial <input> <value>` line names the state or an input that is not carried; an `always` line is not shaped right; or an `always` line names something that is not a carried value, or the state itself. It is because a carried number has both ends of a range that the values a case can hold are finitely many, and the claims about sequences of calls decidable."
            ),
            tr!(
                "持ち越す数には `range >=0 <=7` のように両端のある範囲を書き、どの持ち越す値にも `initial <入力> <値>` で始まりの値を書いてください。`always` には、二行目からの `carry` の左の入力とセルを `and` でつないで書き、成り立つ状態を `when` の後に書きます（`always 残り日数 >=1 <=7 when 猶予中`）。状態について言うことは `never` と `final` で書きます。",
                "Give a carried number a range with both ends, as in `range >=0 <=7`, and every carried value the value it starts at with `initial <input> <value>`. An `always` line names the input on the left of a later `carry` line with a cell, joined by `and`, and the states it holds in after `when` (`always days_left >=1 <=7 when in_grace`). What is said of the state is said with `never` and `final`."
            ),
            X_E072,
            &["E050", "E051", "E135"],
        ),
        err(
            "E075",
            tr!("列挙の値でも、名前でもある語です", "A word is both a value of an enum and a name"),
            tr!(
                "表の出力のセルか `fold` の答えに書いた語が、その列（答え）の列挙の値で、同じ列挙を型にもつ入力・出力・要素のフィールド・導出・定義・表の出力の名前でもあるとき。値としても名前としても読めるので、どちらの意味か決まりません。名前の型が別の型なら、語はその列の値として読みます（数の出力 `gold` があっても、列挙の列に書いた `gold` は列挙の値です）。例と手順の例の期待値のセルは値しか書けないので、いつも値として読みます。",
                "A word in an output cell of a table, or in an answer of a `fold`, is a value of the column's (the answer's) enum, and also the name of an input, an output, a field of an element, a derive, a definition or a table's output whose type is that enum. It reads either way, and which one is meant is not decided. Where the name's type is another, the word is read as the column's value (an output `gold` that is a number does not stop `gold` in a column of the enum from being the enum's value). An expected value in an example or a scenario is always a value, since nothing else can be written there."
            ),
            tr!(
                "列挙の値か名前のどちらかを、別の名前に変えてください。",
                "Rename either the enum's value or the name."
            ),
            X_E075,
            &["E103", "E012"],
        )
        .english(X_E075_EN),
        err(
            "E076",
            tr!("規則が、`implements` で指す契約と合っていません", "The rule does not fit the contract it implements"),
            tr!(
                "`implements proto \"<ファイル>\" <サービス>` を書いた規則が、契約のサービスと合わないとき。契約のサービスは、rulec が規則から生成する `.proto` と同じ形でなければなりません。違ってよいのは、package、サービス・メソッド・メッセージの名前、フィールドの番号と並び、コメント・`reserved`・ほかのオプション、メッセージを置くファイルと入れ子です。合わないところごとに一つ出ます。入力・並び・要素のフィールド・出力が契約に無いとき、契約にしか無いフィールドがあるとき、型が違うとき（真偽は `bool`、日付と文字列は `string`、そのほかの数は `int64`、列挙は規則が `import proto` で読んだその列挙）、`optional` が違うとき（要求と要素のフィールドはいつも `optional`、答えのフィールドは出力が `T?` のときだけ）、通信に乗る列挙が `import proto` で読んだものでないとき、答えに `repeated <メッセージ> trace`（`string table`・`int32 row`・`string label` の三つだけ）が無いとき、サービスの手続きが一つの単項で `NO_SIDE_EFFECTS` のものでないとき、`json_name`・`oneof`・`map` を使っているとき、契約が proto3 でないときです。規則の名前のことはその宣言の行で、契約にしか無いもののことは `implements` の行で言います。",
                "A rule that says `implements proto \"<file>\" <Service>` does not fit the contract's service. The service has to be shaped as the `.proto` rulec generates from the rule; what may differ is the package, the names of the service, the method and the messages, the numbers and the order of the fields, comments, `reserved` and other options, and the file and the message a message is declared in. Each thing that does not fit is said once: an input, a sequence, a field of an element or an output the contract does not have; a field only the contract has; a field of another type (a truth value is `bool`, a date and a string are `string`, every other number is `int64`, and an enum is the very enum the rule reads with `import proto`); a field whose `optional` differs (every field of the request and of an element is `optional`, and a field of the answer is only for an output of `T?`); an enum on the wire that is not read with `import proto`; an answer with no `repeated <Message> trace` of `string table`, `int32 row` and `string label` alone; a service whose one method is not unary with `NO_SIDE_EFFECTS`; a field with `json_name`, in a `oneof`, or a `map`; and a contract that is not proto3. A name of the rule's is said at its declaration, and what only the contract has at the `implements` line."
            ),
            tr!(
                "契約にフィールドを足すときは、そのメッセージのどのフィールドもまだ使ったことのない番号を付けてください。契約から消したフィールドの番号は `reserved` にしてください。規則の側は、値の別名を契約のフィールドの名前にしてください。受け取るだけで使わない値は、規則に `T?` の入力として書き、`contract_only` を付けてください。通信に乗る列挙は、契約の `.proto` に書いて、`import proto` で読んでください。初めて契約を作るなら、`rulec gen` が書いた `.proto` を写すと、そのまま合います。",
                "When the contract gains a field, give it a number no field of its message has ever used; when it drops one, make the number `reserved`. On the rule's side, the alias of a value is the name of the contract's field. A value the service only takes and never reads is an input of `T?` marked `contract_only`. An enum on the wire is declared in the contract's `.proto` and read with `import proto`. To make a first contract, copy the `.proto` that `rulec gen` writes: it fits as it stands."
            ),
            X_E076,
            &["E013", "E032", "E033"],
        )
        .with_files(SHIPPING_PROTO)
        .also_with(tr!("契約にしか無いフィールド", "A field only the contract has"), X_E076_BASE, SHIPPING_EXTRA)
        .also_with(tr!("型の違うフィールド", "A field of another type"), X_E076_BASE, SHIPPING_TYPE)
        .also_with(tr!("`optional` の違うフィールド", "A field whose `optional` differs"), X_E076_BASE, SHIPPING_PRESENCE)
        .also_with(tr!("契約から読んでいない列挙", "An enum not read from the contract"), X_E076_ENUM, SHIPPING_ENUM)
        .also_with(tr!("`trace` の無い答え", "An answer with no `trace`"), X_E076_BASE, SHIPPING_NO_TRACE)
        .also_with(tr!("規則のサービスの形でないサービス", "A service not shaped as a rule's"), X_E076_BASE, SHIPPING_SIDE_EFFECTS)
        .also_with(tr!("`json_name`・`oneof`・`map` のフィールド", "A field with `json_name`, in a `oneof`, or a `map`"), X_E076_BASE, SHIPPING_JSON_NAME)
        .also_with(tr!("proto3 でない契約", "A contract that is not proto3"), X_E076_BASE, SHIPPING_PROTO2),
        err(
            "E101",
            tr!("完全性の欠落: どの行にも当てはまらない入力があります", "Completeness gap: some input matches no row"),
            tr!(
                "行を全部合わせても、宣言した範囲の入力を覆いきれていないとき。完全性は宣言で外せず、常に必須です。当てはまらない入力の具体例が必ず付きます。列が規則の計算する値（導出、`define`、上の表の出力）なら、例はその値で書かれ、その値を作る入力も、参照の評価器で確かめられたときは付きます。その入力は、導出が足し算・引き算・定数倍だけなら式から求め、ほかの列（`min` や丸めを使う導出、`define`、上の表の出力）では、入力の組み合わせが 16,384 通りまでなら全部を確かめて探します。導出の値は、セルを書く単位より細かい刻みをとることがあり（`amount * 10%` は 0.1 刻み）、その刻みで調べます。どの入力も届かない値には、行を求めません。導出と、数を計算する `define` の列では、その列が実際に取りうる値の外に、行を求めません（取りうる値は、式を入力の範囲と上流の表が出す値で計算したもので、導出では `constraint` も読みます）。上流の表が出す値の列では、上流の表がその値を出す行の条件と合わせて、ほかの列が取りえない値に、行を求めません。一つの比べ方でできた真偽の `define` の列では、入力の範囲でいつも同じ値をとるなら、もう一方の値に行を求めません（`作成日 <= 2027-03-31` で、作成日がその日までしかとらないなら、false に行を求めません）。optional の入力（`T?`）では、値が無いことも入力の一つで、その行を求めます。数・日付・真偽の optional の入力では、値があるときの宣言した範囲の全部にも行を求めます。上の表が決める optional の列では、上の表が `none` を書く行か、値の無いことのある名前を渡す行があれば、値の無いことにも行を求めます。",
                "The union of the rows does not cover the declared input space. Completeness cannot be waived and is always required. A concrete input that matches no row is always attached. Where a column is a value the rule computes (a derive, a `define`, the output of a table above), the example gives its value, and the input that produces it too, once the reference evaluator has confirmed it. That input is worked out from the expression where a derive only adds, subtracts and multiplies by constants, and otherwise (a `min` or a rounding, a `define`, a table above) looked for among every combination of the inputs when there are no more than 16,384. A derive's values can fall finer than the units a cell is written in (`amount * 10%` comes in tenths), and they are read on that step. No row is asked for where no input reaches: on the column of a derive or of a `define` that computes a number, past what it can actually come to (its expression computed over the inputs' ranges and the values the tables above produce, and for a derive over the `constraint` lines too); on a column a table above decides, at values of the other columns that the rows writing its value never let come with it; on the column of a boolean `define` of one comparison, at the truth value it never takes over the inputs' ranges (`made <= 2027-03-31` with `made` declared only up to that day is never false). An optional input (`T?`) takes the absent value too, and a row is asked for it; an optional number, date or truth value is asked for the whole of its declared range besides. An optional column a table above decides takes the absent value where a row above writes `none` or passes on a name that may have none, and a row is asked for it there."
            ),
            tr!(
                "それを起こす入力に当てはまる行を足してください。列挙の値が増えたのが原因なら、その値の行か、全部を受ける `-` の行を足してください。値に専用の行が要らないなら、列挙の宣言に `default` を付けてください。",
                "Add a row that matches the witness. If a new enum value caused it, add a row for that value or a `-` row that catches everything. If the value needs no row of its own, mark it `default` in the enum declaration."
            ),
            X_E101,
            &["E102", "E105", "W111"],
        ),
        err(
            "E102",
            tr!("どの入力にも当てはまらない行があります", "Unreachable row: the row never matches"),
            tr!(
                "どの入力もその行に当てはまらないとき。形は三つあり、診断の文が原因を書き分けます。一つ目は、先行する行（`overrides` で優先する行も含む）にすべて覆われている行です。二つ目は、上流の表が決して出さない値を指している行です。三つ目は、導出か、数を計算する `define` の列に、その列が実際に取りうる値の外だけを求めている行です（取りうる値の中の部分を、先行する行が先に取っている場合も含みます）。上流の表が出す値と導出の両方を列に持つ表で、上流の表がその値を出すときには導出が取りえない値を求めている行と、一つの比べ方でできた真偽の `define` が、入力の範囲ではとらない値を求めている行も、この形です。取りうる値は、式を入力の範囲と上流の表が出す値で計算したもので（導出では `constraint` も読みます）、導出に書いた `range` ではありません。",
                "No input matches the row. There are three forms, told apart in the wording. In the first, every input the row would take is already taken by an earlier row, or by a row that takes precedence through `overrides`. In the second, the row names a value that the upstream table never produces. In the third, the row asks a derived column, or the column of a `define` that computes a number, only for values outside what it can actually reach, the part inside (if any) being taken first by earlier rows; so does a row that asks a column a table above decides and a derived column for values the derive never comes to while the rows that write that value fire, and a row that asks a boolean `define` of one comparison for the truth value it never takes over the inputs' ranges. What a value can reach is its expression computed over the inputs' ranges and the values the tables above produce (for a derive, over the `constraint` lines too), not the `range` written on it."
            ),
            tr!(
                "その行が新しい仕様なら、覆っている行より上へ移してください。不要なら削除してください。上流が出さない値を指しているなら、上流の表にその値を出す行を足すか、この行を消してください。導出や `define` が取りえない値を求めているなら、条件をその取りうる値の中に書き直すか、この行を削除してください。導出の `range` を広げても、取りうる値は変わりません。真偽の `define` がとらない値を求めているなら、この行の条件を、その `define` がいつもとる値に書き直すか、この行を削除してください。",
                "If the row is the newer intent, move it above the row that covers it. If it is dead, delete it. If it names a value the upstream never emits, either add a row upstream that emits it, or delete this row. If it asks a derive or a `define` for values it never reaches, rewrite the condition within the values it can come to, or delete the row; widening a derive's `range` does not change those values. If it asks a boolean `define` for the truth value it never takes, rewrite the condition to the value it always takes, or delete the row."
            ),
            X_E102,
            &["E101", "W105", "W110", "E112"],
        )
        .also(
            tr!("三つ目の形（導出の取りうる値の外にある行）の最小の再現", "Smallest reproduction of the third form (a row outside what a derive can reach)"),
            X_E102_REACH,
            X_E102_REACH_EN,
        )
        .also(
            tr!("三つ目の形（`define` の取りうる値の外にある行）の最小の再現", "Smallest reproduction of the third form (a row outside what a `define` can reach)"),
            X_E102_DEFINE,
            X_E102_DEFINE_EN,
        )
        .also(
            tr!(
                "三つ目の形（上流の表がその値を出すときには、導出が取りえない値を求める行）の最小の再現",
                "Smallest reproduction of the third form (a row asking a derive for values it never comes to while a table above writes the value the row asks for)"
            ),
            X_E102_ABOVE,
            X_E102_ABOVE_EN,
        )
        .also(
            tr!(
                "三つ目の形（真偽の `define` が入力の範囲ではとらない値を求める行）の最小の再現",
                "Smallest reproduction of the third form (a row asking a boolean `define` for the truth value it never takes)"
            ),
            X_E102_TRUTH,
            X_E102_TRUTH_EN,
        ),
        err(
            "E103",
            tr!("単位の混同: 型の違う値を混ぜています", "Unit mismatch: values of different types are being mixed"),
            tr!(
                "式やセルで、単位・通貨・税区分の違う値を足したり比べたりしているとき。`money[円, incl_tax]` と `money[円, excl_tax]` も別物です。型の刻みが読めないとき（`rate[step 1g]`）と、率の入力に刻みが無いときにも出ます。どちらも、実行時に受け渡す整数が何を単位に数えているのかが決まりません。お金の型の税区分に `incl_tax` と `excl_tax` のほかの語を書いたとき（`money[円, foo]`）にも出ます。列の列挙に無い値やグループをセルに書いたときにも出ます。出力のセルに書いたときと、グループが二つの列挙の値を混ぜているときも同じです。式の中の数が、書いた単位の値にならないときにも出ます。単位の無い小数（`amount * 0.4`。単位の無い数は整数の `number` です）と、金額や量の端数（`0.5円`）です。表・節・例の見出しに、その名前がもう持っている型と違う型を書いたときにも出ます。出力の名前なら出力の宣言の型、上の表や節が決める名前ならその型です。optional でない列（規則の出力、表が決める列、例の期待値）に `none` を書いたときと、値の無いことのある名前を書いたときにも出ます。値の無いことのある名前を式の中で使ったとき（`quoted * 2`）にも出ます。そういう値は、表のセルで確かめるか、optional の列にそのまま渡すことしかできません。",
                "An expression or a cell adds or compares values whose unit, currency or tax flag differ. `money[JPY, incl_tax]` and `money[JPY, excl_tax]` are different types too. It is also a step that cannot be read (`rate[step 1g]`), and a rate input that declares no step: either way, what the integer passed at runtime counts is not settled. So is a money type whose tax is a word other than `incl_tax` and `excl_tax` (`money[JPY, foo]`). And it is a value or a group written in a column whose enum does not have it, an output cell included, and a group that mixes the values of two enums. And it is a number in an expression that is no value of the unit it is written in: a bare decimal (`amount * 0.4`; a number with no unit is a `number`, which is whole) and a fraction of a money or quantity unit (`0.5JPY`). And it is the heading of a table, a clause or examples that writes a type other than the one its name already has: the declared type of an output, or the type a table or clause above decides the name in. And it is `none`, or a name that may have no value, written under a column that is not optional: an output, a column a table decides, an expected value. And it is a name that may have no value worked with in an expression (`quoted * 2`): such a value is tested in a table's cells, or handed on to an optional column as it is, and nothing else."
            ),
            tr!(
                "混ぜている片方を表に移してください。「重量に応じた加算料金」なら `table 重量加算 | 重量 | -> 加算額 : money[円, incl_tax] |` の形です。税の変換も、式ではなく表として書いてください。刻みなら、型と同じ単位で書いてください（率の入力は `rate[step 1%]` や `rate[step 0.1%]`）。税区分は、税込なら `incl_tax`、税抜なら `excl_tax` と書いてください。率は `%` で書いてください（`0.4` は `40%`。`fix.text` がこの形を出します）。見出しの型が名前の型と違うなら、どちらが正しいかを決めて直してください。見出しの型は消してもかまいません。消せば、名前の型で読みます。値の無いことを答える列なら、その型を `T?` にしてください（規則の出力なら `outputs` の宣言、表が決める列なら見出しの型です）。そうでなければ、`none` の代わりに値を書いてください。",
                "Move one side into a table. \"A surcharge that depends on weight\" is `table weight_surcharge | weight | -> surcharge : money[JPY, incl_tax] |`. A tax conversion is also written as a table, never as a formula. A step is written in the unit of its type (a rate input takes `rate[step 1%]` or `rate[step 0.1%]`). A tax is `incl_tax` with tax and `excl_tax` without. A rate is written with `%` (`0.4` is `40%`, which `fix.text` gives). Where a heading's type is not its name's, decide which is right and correct the other; a heading may also go without a type, and is then read in the name's. Where a column is to answer that there is no value, make its type `T?`: the declaration under `outputs` for an output, the heading for a column a table decides; otherwise write a value in the place of `none`."
            ),
            X_E103,
            &["E108", "E112"],
        )
        .english(X_E103_EN)
        .also(
            tr!("出力と違う型を書いた、表の見出し", "A table's heading that writes a type other than the output's"),
            X_E103_HEADING,
            X_E103_HEADING_EN,
        )
        .also(tr!("optional でない出力に書いた `none`", "`none` under an output that is not optional"), X_E103_NONE, X_E103_NONE_EN),
        err(
            "E104",
            tr!("数値の出力に丸めの宣言がありません", "A numeric output declares no rounding"),
            tr!(
                "数量・金額・率の出力に `round` が無いとき。optional の出力（`money[円]?` など）も同じです。端数がどう決まるかを宣言しないと、生成コードが黙って決めてしまいます。式が端数を生む場合は、丸め方で答えがどれだけ動くかが、診断に数字で出ます。",
                "A quantity, money or rate output has no `round`, an optional one (`money[JPY]?`) included. Unless the fraction is declared, the generated code settles it silently. When the expression can produce a fraction, the message shows in yen how far the choice moves the answer."
            ),
            tr!(
                "出力の宣言に丸めを書いてください。例: `round up(10円)`。向きは五種（`up` `down` `half_up` `half_down` `half_even`）で、負の側まで固定されています。",
                "Add rounding to the output declaration, e.g. `round up(10JPY)`. There are five directions (`up`, `down`, `half_up`, `half_down`, `half_even`), pinned down for negative values as well."
            ),
            X_E104,
            &["E106", "E103"],
        )
        .english(X_E104_EN),
        err(
            "E105",
            tr!("行の重なり: 同じ入力が二つ以上の行に当てはまります", "Overlapping rows: the same input matches two or more rows"),
            tr!(
                "`policy unique` の表で、両方に当てはまる入力を実際に構成できたとき。構成できなかった重なりは W114 に落ちます。上の表が同時には出さない値の組でしか重ならない対と、どの入力も届かない点（`define` の取りうる値の外、真偽の `define` がとらない値、上の表がその値を出す行の条件と導出の組み合わせ）でしか重ならない対は、重なりとして報告しません（完全性の検査と E102 が、そういう点に行を求めず、そこだけを求める行をどの入力にも当てはまらない行とするのと同じ読み方です）。optional の列では、比べ方、リテラル、値の集合は値があるときだけに当てはまり、`-` と `not:` は値が無いことにも当てはまります。だから `none` の行は、`-` の行とも `not: 5` の行とも、値の無い入力で重なります。",
                "In a `policy unique` table, an input matching both rows was actually constructed. An overlap that could not be constructed falls to W114 instead. A pair that meets only on a combination the tables above never produce together, or only where no input reaches — past what a `define` comes to, at the truth value a boolean `define` never takes, where the rows of a table above leave a derive no values — is not reported: the completeness check asks no row there, and E102 calls a row that asks only for such points dead. On an optional column a comparison, a literal and a set of values match only a value that is there, while `-` and `not:` match the absent value too; so a `none` row meets a `-` row, and a `not: 5` row, on the absent value."
            ),
            tr!(
                "出力が違うなら、どちらが正しいか決めて行を直してください。順序に意味を持たせたいなら `policy first` を宣言してください。出力まで同じなら、片方を削ってください。同じ出力を定める二つの表や節が重なるときは、勝つほうに `overrides <相手>` を書いてください。表なら `policy` の次の行に、節なら節の下の行に書きます。二つが同じ答えを返していても、重なりは許しません。今日同じ答えでも、明日の編集で片方だけが変わるからです。",
                "If the outputs differ, decide which is right and fix the rows; to let the order decide, declare `policy first`. If even the outputs agree, delete one of the rows. Where two tables or clauses that define one output overlap, write `overrides <the other>` on the one that wins: after `policy` on a table, as a line under a clause. The overlap is refused even when the two answer the same, since an edit tomorrow may change one of them and not the other."
            ),
            X_E105,
            &["W105", "W114", "E102"],
        )
        .english(X_E105_EN),
        err(
            "E106",
            tr!("出力のリテラルが丸めの刻みに載っていません", "An output literal is not on the rounding grid"),
            tr!(
                "出力セルに書かれたリテラルが、宣言した丸めの刻みの倍数でないとき。`round up(10円)` の表に `1451円` があるような、桁の打ち間違いをここで落とします。",
                "A literal in an output cell is not a multiple of the declared rounding grid. This is where a mistyped digit — `1451JPY` in a table rounded `up(10JPY)` — is stopped."
            ),
            tr!(
                "リテラルを刻みに載せてください（`1451円` は `1450円` か `1460円`）。その値が本当に正しいなら、丸めの刻みのほうを直してください。",
                "Put the literal on the grid (`1451JPY` becomes `1450JPY` or `1460JPY`). If the amount really is right, change the grid instead."
            ),
            X_E106,
            &["E104"],
        )
        .english(X_E106_EN),
        err(
            "E107",
            tr!("例の期待値と一致しません", "An example does not match"),
            tr!(
                "`examples` の行を参照評価器で走らせた結果が、書かれた期待値と違うとき。**どの表のどの行に当てはまったか**が付きます。`examples` は実行される仕様です。optional の出力の期待値の `none` は、値の無いことを期待します。",
                "Running a row of `examples` through the reference evaluator gives something other than the value written. **Which row of which table fired** is attached. `examples` is an executable specification. An expected `none` under an optional output expects no value."
            ),
            tr!(
                "表が正しいなら期待値を直してください。期待値のほうが正しいなら、当てはまった行を直してください。どちらを直すかは、出典（規約、Excel、いま動いている実装）が決めます。",
                "If the table is right, fix the expected value. If the expected value is the business truth, fix the row that fired. Which one to fix is settled by the source: the written rule, the spreadsheet, or the legacy implementation."
            ),
            X_E107,
            &["E111", "E105"],
        )
        .english(X_E107_EN),
        err(
            "E108",
            tr!("中間値が int64 に収まることを証明できません", "Cannot prove an intermediate value fits in int64"),
            tr!(
                "宣言した範囲と刻みから計算した「実際に取りうる値」が、int64 を超えるとき。率の刻みが 1% なら格納される整数は 100 倍になります。乱数の入力（`random`）の点を掛け合わせた数が 2^62 を越えるときもこれです。確率は点を数えて出し、数えた通りの数はどの言語でも int64 で持ちます。",
                "The reachable interval computed from the declared ranges and steps exceeds int64. With a rate step of 1%, the stored integer is 100 times the value. So is a rule whose random inputs (`random`) have more than 2^62 points together: a chance is worked out by counting the points, and every language holds the count in an int64."
            ),
            tr!(
                "入力の範囲を狭めるか、途中に丸めを一つ入れてください。どこで丸めるかは答えを変える業務の判断なので、rulec は決めません。乱数の入力の点が多すぎるなら、その範囲を狭めてください。",
                "Narrow the input ranges, or insert one rounding step along the way. Where to round is a business decision that moves yen, so the tool does not decide it. Where the random inputs have too many points, narrow their ranges."
            ),
            X_E108,
            &["E112", "E103"],
        )
        .english(X_E108_EN)
        .also(tr!("乱数の入力の点", "The points of the random inputs"), X_E108_RANDOM, X_E108_RANDOM),
        err(
            "E109",
            tr!("検査の予算を超えたので、完全性を証明できませんでした", "The check exceeded its budget, so completeness could not be proven"),
            tr!(
                "領域検査が訪れたノード数が `--budget` を超えたとき。**証明できなかったことを緑にはしない**ので、警告ではなくエラーです。",
                "The region check visited more nodes than `--budget` allows. Failing to prove something is never green here, so this is an error and not a warning."
            ),
            tr!(
                "表を分けて列の数を減らすか、`--budget` を上げてください（`rulec check` にも `ritsu check` にもあります）。列の積が効くので、一つの表に列を積むより、表を一列につないでいくほうが安く済みます。",
                "Split the table to reduce the number of columns, or raise `--budget` (both `rulec check` and `ritsu check` take it). The cost is the product of the columns, so chaining tables in a linear pipeline is cheaper than piling columns into one table."
            ),
            X_E109,
            &["E101", "W114"],
        )
        .with_budget(1),
        err(
            "E110",
            tr!("検査できない型の列があります", "A column has a type the check cannot handle"),
            tr!(
                "その列の型を、区画の計算ができる形に落とせないとき。**これが出たら rulec 自身のバグです。** 表の検査が黙って素通りするのを防ぐための内部の検査で、日付と optional で二度起きた同じ種類の事故を、まとめて塞いだものです。",
                "The column's type cannot be lowered into the region IR. **Seeing this is a bug in rulec itself.** It is the internal breakwater that stops a table from being skipped silently, put in after the same accident happened twice, with dates and with optional."
            ),
            tr!(
                "その列を、いま検査できる型（真偽・列挙・数量・金額・率・日付・optional）に直してください。そのうえで報告してください。黙って素通りするより止まるほうが正しい、という判断でこの検査を置いています。",
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
        )
        .english(X_E111_EN),
        err(
            "E112",
            tr!("導出の範囲が、実際に到達しうる値を含んでいません", "The range of a derived value does not contain the values it can reach"),
            tr!(
                "入力の範囲から計算した「実際に取りうる値」が、導出に宣言した `range` からはみ出すとき。範囲が狭いと、完全性検査が実際に起きる値を見ないまま「完全」と答えます。`max`・`min` に書いた `range` が、列の範囲と `empty` の値を含まないときも、これです。答えはそのどちらかの値だからです。",
                "The interval computed from the input ranges falls outside the `range` declared on the derived value. With too narrow a range, the completeness check answers \"complete\" without ever looking at values that really occur. So does a `range` written on a `max` or a `min` that leaves out part of its column's range or its `empty` value, since it answers one of those."
            ),
            tr!(
                "文面が示す範囲まで `range` を広げてください（`range >=-110万円 <=100万円` の形で書いてあります）。起こりえない分まで広げても、検査がそこを自分で外すので害はありません。",
                "Widen the `range` to the reachable interval the message states (it is written out, e.g. `range >=-1_100_000JPY <=1_000_000JPY`). Widening past what is reachable costs nothing: the check sifts the infeasible part out."
            ),
            X_E112,
            &["E108", "E101"],
        )
        .english(X_E112_EN),
        err(
            "E113",
            tr!("真偽定義の条件が、書ける二つの形のどちらでもありません", "The condition of a boolean definition is neither of the two allowed forms"),
            tr!(
                "`define … : bool` の条件が、「入力か導出の値ひとつを定数と比べる」形でも、「引き算で差を取れない型どうしの比較」（日付どうしなど）でもないとき。数値どうしを直接比べたときがこれに当たります。",
                "The condition of `define … : bool` is neither one input or derived value compared with a constant, nor a comparison of two values whose difference cannot be subtracted (two dates, say). Comparing two numbers directly is the usual case."
            ),
            tr!(
                "差を導出として宣言してから定数と比べてください。`define bigger : bool = a >= b` は `derive gap(gap) : money[円, incl_tax] = a - b  range …` を足して `| gap | >=0円 |` と書き換えられます。そのほうが厳密に解析できます。",
                "Declare the difference as a derived value and compare that against a constant. `define bigger : bool = a >= b` becomes `derive gap(gap) : money[JPY, incl_tax] = a - b  range …` and the cell `>=0JPY`. The analysis is exact that way."
            ),
            X_E113,
            &["E112", "E103"],
        )
        .english(X_E113_EN),
        err(
            "E114",
            tr!("値が宣言した刻みに載っていません", "A value does not sit on the declared step"),
            tr!(
                "`rate[step 1%]` の列に `0.5%` のように、宣言した刻みの整数倍でない値が書かれたとき。実行時の値はその刻みの整数一本なので、この値には表し方がありません。率の出力の丸めの刻みが、出力の宣言した刻みの整数倍でないとき（`rate[step 1%]` に `round down(0.5%)`）も同じです。答えは丸めの刻みに載り、呼び出し側へは宣言した刻みの整数で渡すので、表せない答えが出ます。",
                "A value that is not a whole number of the declared step is written in the column, such as `0.5%` where the type says `rate[step 1%]`. At runtime the value is one integer count of that step, so this one has no representation. The same holds for the rounding grid of a rate output that is not a whole number of the step the output declares (`rate[step 1%]` with `round down(0.5%)`): every answer sits on the grid and is handed over as a whole number of the step, so some answers could not be written."
            ),
            tr!(
                "刻みに載る値に直すか、型の刻みを細かくしてください（`rate[step 0.1%]`）。黙って近い刻みに寄せると、表で読める境界と生成コードの境界が食い違います。丸めの刻みなら、刻みを丸めに合わせるか、丸めの刻みを刻みの整数倍にしてください。",
                "Write a value on the step, or declare a finer step (`rate[step 0.1%]`). Quietly moving it to the nearest step would make the boundary on the page differ from the boundary in the generated code. For a rounding grid, declare the step the rounding needs, or round to a whole number of the step."
            ),
            X_E114,
            &["E103", "E106"],
        ),
        err(
            "E115",
            tr!("割る数が正の定数ではありません", "The divisor is not a positive constant"),
            tr!(
                "`÷` の右が、正の整数の定数でも、同じ単位の正の金額・数量の定数でもないとき。変数、0、負の数、小数がこれに当たります。ただし、丸めの関数の引数のいちばん外側の割り算は、値で割れます（`half_up(合計 / 件数, 1円)`。割る数が 0 にならないことは E132 が確かめます）。`mod(x, n)` の割る数 n も、正の整数の定数でなければこれです。",
                "The right of `÷` is neither a positive whole constant nor a positive constant amount or quantity in the same unit: a variable, zero, a negative number or a fraction. A division that is the outermost operation of a rounding function's argument may divide by a value (`half_up(total / units, 1JPY)`; E132 sees that the divisor is never zero). The divisor n of `mod(x, n)` is this too when it is not a positive whole constant."
            ),
            tr!(
                "値で割るなら、割り算を丸めの関数の中に入れて、刻みと丸め方を書いてください（`half_up(合計 / 件数, 1円)`）。割る数が業務のデータで率として渡せるなら、率の入力にするか、定数を引く表として書いてください。刻みが静的に決まらないと生成コードは言語の除算に頼ることになり、Python は −∞ 方向、Go は 0 方向に丸めて答えが食い違います。",
                "To divide by a value, put the division inside a rounding function, with the grid and the way to round (`half_up(total / units, 1JPY)`). If the divisor is business data that can come as a rate, take it as a rate input, or look the constant up in a table. Without a statically known step the generated code falls back on the language's own division, and Python rounding toward -inf and Go toward zero disagree."
            ),
            X_E115,
            &["E103", "E108"],
        ),
        err(
            "E116",
            tr!("行の金額が、引いた出典のコピーに無いか、別の見出しの下にあります", "A row's amount is not in the copy it cites, or is under another heading there"),
            tr!(
                "行の出力の値が、その行（またはその表）が `@出典 表1` で引いている出典のコピーのどこにも出てこないとき。行のセルの語（`関東`）と一字一句同じ見出しがコピーにあれば、その見出しの行と列の中だけを探します。隣の行の金額はコピーのどこかにはあるので、表全体を探したのでは取り違えを見逃すからです。比べるのは金額だけです。閾値は転記するときに書き換わります（`1,949,000円まで` は `<=1949000円` になる）が、金額は書き換わらないからです。rulec は、コピーの `5/1,000` や `1,000分の5` を 0.5% と読みます。コピーは `rulec source fetch` が文書から取り出して保存したもので、check が見るのはそのコピーであって文書そのものではありません。",
                "The output value of a row is nowhere in the copy the row or its table cites with `@source table1`. Where the copy has a heading that says exactly a word of the row's cells (`Kanto`), only the row and the column under that heading are searched: the amount of the next row is somewhere in the copy too, and a search of the whole table would let the two be mixed up. Only amounts are compared: a threshold is rewritten as it is transcribed (`up to 1,949,000JPY` becomes `<=1949000JPY`) and an amount is not. A copy's `5/1,000` (or, in Japanese, `1,000分の5`) reads as 0.5%. The copy is what `rulec source fetch` took out of the document; check does not read the document itself."
            ),
            tr!(
                "出典のコピーを読み直して金額を直してください。別の見出しの下にあると言われたら、行を取り違えています。一桁の打ち間違いなら、たいてい同時に W120 が出て、どの値が使われずに残っているかを言います。値が別のところ（後の通知、正誤表、人の回答）から来たのなら、この行の引用を外し、どこから来たかを行末のコメントに書いてください。`rulec doc` がそのコメントを、規則を読む人に見せます。",
                "Reread the copy and correct the amount. When it is said to be under another heading, the rows were mixed up. For a mistyped digit W120 usually comes with it, naming the value left unused. If the value came from somewhere else — a later notice, a correction, an answer from a person — take the citation off this row and write where it came from in a comment at the end of it, which `rulec doc` shows on the page for people."
            ),
            X_E116,
            &["W120", "E038", "E107"],
        )
        .with_files(TARIFF_DOC)
        .english_with_files(X_E116_EN, TARIFF_DOC_EN),
        err(
            "E117",
            tr!("配分の前提が揃っていません", "A share without what a share needs"),
            tr!(
                "`allocate(配る額, 累計, 全体)` の三つが配分の形になっていないとき。三つとも範囲を宣言した名前で、配る額と累計は負になれず、全体は正で、累計が全体を超えないと `constraint` が言っていなければなりません。",
                "The three of `allocate(<amount>, <running total>, <whole>)` are not the shape a share needs. All three are names with declared ranges, the amount and the running total cannot be negative, the whole is positive, and a `constraint` says the running total never passes the whole."
            ),
            tr!(
                "足りないものを書いてください。`constraint 累計 <= 全体` がないと、配る分が配る額を超えることがあり、明細の合計が総額に一致しません。負の値が混じると、下へ丸めるのか 0 の方向へ丸めるのかで、言語ごとに答えが割れます。",
                "Write what is missing. Without `constraint <running total> <= <whole>` a share can exceed the amount being handed out and the lines no longer add up to the total. Below zero the targets disagree about which way to round."
            ),
            X_E117,
            &["E115", "E108"],
        )
        .english(X_E117_EN),
        err(
            "E118",
            tr!("呼び出しの形が違います", "The call is not written correctly"),
            tr!(
                "無い関数を呼んでいるか、引数の数が合わないとき。書けるのは `min(a, b)` `max(a, b)` `allocate(配る額, 累計, 全体)` `mod(x, n)` `isqrt(x)` と丸めの五つ（`down(x, 1円)` など）だけです。丸めの刻みが定数でないときも、これです。",
                "A call to a function that does not exist, or with the wrong number of arguments. The calls are `min(a, b)`, `max(a, b)`, `allocate(<amount>, <running total>, <whole>)`, `mod(x, n)`, `isqrt(x)` and the five rounding modes (`down(x, 1JPY)` and the rest). A rounding grid that is not a constant is this too."
            ),
            tr!(
                "綴りと引数の数を確かめてください。引数が多ければ余りは黙って捨てられ、少なければ答えが決まりません。どちらも以前の版では検査を素通りしていて、ジェネレーターで初めて止まっていました。",
                "Check the spelling and the count. A spare argument is dropped on the floor and a missing one leaves no answer — both used to pass unnoticed, and only the generator ran out of cases."
            ),
            X_E118,
            &["E103", "E115"],
        ),
        err(
            "E120",
            tr!("`from` が入力の型と合いません", "A `from` does not fit the input's type"),
            tr!(
                "`from` の返すものが、それを受ける入力の型と合わないとき。`any` と `all` は `bool` を、`count` は `number` を、`sum` は数（`number`・お金・量・率）を、`max` と `min` は数か日付を返します。パスの先にあるものの型が入力と合わないとき（契約が文字列と言っているフィールドを `number` の入力で受けるなど）と、`any`・`all`・`count`・`sum`・`max`・`min` が並びでないものを歩こうとしているとき、`where` の値がフィールドの型と合わないときにも出ます。`of` が読む要素のフィールドの型が入力と合わないとき（`sum` は整数を足し、小数は足せません）、契約から取る並びが配列でないとき、並びの要素のフィールドの型が合わないときも同じです。契約が決めるのは値の運ばれ方なので、列挙も日付も文字列で、金額と数量は宣言した単位の整数で届きます。",
                "What a `from` yields does not fit the input that takes it. `any` and `all` yield a `bool`, `count` yields a `number`, `sum` a number (a `number`, money, a quantity or a rate), and `max` and `min` a number or a date. It is also this code when the type at the end of the path does not fit the input — a field the contract calls a string taken by a `number` input — when `any`, `all`, `count`, `sum`, `max` or `min` would walk something that is not a collection, and when the value of a `where` does not fit the field; when the field of an element `of` reads does not fit the input (`sum` adds whole numbers, and a fraction cannot be added up exactly); when a sequence is taken from something that is not a collection; and when a field of an element does not fit the field it is read into. A contract says how a value travels: an enum and a date arrive as strings, and money and a quantity as whole numbers in the unit the rule declares."
            ),
            tr!(
                "型のほうか `from` のほうを直してください。件数が欲しいなら `number` の入力に範囲を付けて受け、当てはまるかどうかが欲しいなら `bool` で受けてください。値そのものが欲しいなら `from <shape の名前>.<フィールド>` と書いてください。`where` の値に単位は書けません。契約の値には単位が無いので、書けば目盛りの違う数どうしを黙って比べることになるからです。",
                "Correct the type or the `from`. A count is taken by a `number` input with a range, whether the elements passed by a `bool`, and the value itself by `from <shape>.<field>`. A `where` value carries no unit: a contract has none, and comparing a scaled number with a raw one is the thing this must not do quietly."
            ),
            X_E120,
            &["E121", "W122", "E103"],
        )
        .with_files(SHAPE_DOC)
        .english(X_E120_EN),
        err(
            "E121",
            tr!("`from` のパスが契約にありません", "The contract has no such path"),
            tr!(
                "`from` のパスが、宣言した `shape` の契約の中に見つからないとき。パスの最初の語が `shape` の名前でないとき、途中のフィールドが無いとき、`where` や `of` の見るフィールドが要素に無いときです。`elements … from` の並びのパスと、その要素のフィールドのパス（`order.lines[].qty` のように、要素の中を `[]` の後ろに示します）も同じです。注に、パスがどこまで届いたかと、そこにあったフィールドの名前が出ます。パスが並び（JSON Schema の `array`、`.proto` の `repeated` のフィールド）の先へ続くときも、このエラーです。パスが読むのは一つの値で、要素はそれぞれ自分の値を持つので、入力がどの要素のものを受け取るかが決まりません。注は、パスが通る並びを全部挙げます。契約は `.proto` でも JSON Schema でもよく、`import proto` と同じく毎回の `check` で読まれ、固定は付きません。",
                "A `from` path is not in the contract of the `shape` it starts at: the first word is not the name of a `shape`, a field along the way is not there, or the field a `where` tests or an `of` reads is not a field of an element. The same goes for the path of a sequence taken with `elements … from`, and for the path of each field of its elements, shown after `[]` inside the element (`order.lines[].qty`). The message says how far it resolved and which names were there. A path that goes on through a collection — an `array` in a JSON Schema, a `repeated` field in a `.proto` — is this code too: a path reads one value, and each element has its own, so which one the input takes is not said. The message names every collection along the path. The contract may be a `.proto` or a JSON Schema, is read on every `check` like `import proto`, and carries no pin."
            ),
            tr!(
                "綴りを直すか、契約のほうが動いたのならパスを書き直してください。**このエラーはそのためにあります。** 契約がフィールドの名前を変えたとき、手書きのつなぎのコードなら実行するまで気づきませんが、rulec は生成の前に止まります。パスが並びを通るときは、並びを読む書き方にしてください。要素のフィールドを調べる `from any <並び> where <フィールド> = <値>`（`all`、`count` も）、数を足し合わせたり比べたりする `sum`・`max`・`min`、要素を一つずつたどる `elements <名前> from <並び>` です。並びに要素がいつも一つなら、契約のほうで、それを一つのオブジェクト（`repeated` でないフィールド）にしてください。",
                "Correct the spelling, or rewrite the path if the contract moved. **This firing is the point**: a contract that renamed a field goes unnoticed in hand-written glue until it runs, and stops the build here. Where the path goes through a collection, read the collection instead: test a field of its elements with `from any <collection> where <field> = <value>` (or `all`, `count`), add up or compare a number with `sum`, `max` or `min`, or walk the elements with `elements <name> from <collection>`. Where the collection always holds one element, make it one object in the contract (a field that is not `repeated`)."
            ),
            X_E121,
            &["E120", "W122", "E032"],
        )
        .with_files(SHAPE_DOC)
        .english(X_E121_EN)
        .also_with(tr!("並びの先へ続くパス", "A path that goes on through a collection"), X_E121_THROUGH, SHAPE_DOC),
        err(
            "E119",
            tr!("行の境界が、引いた出典のコピーと反対側です", "A row's boundary falls on the other side from the copy it cites"),
            tr!(
                "引用のある行の閾値が、境界の値をコピーと反対の側に入れているとき。閾値は転記するときに書き換わる（`1,949,000円まで` は `<=1949000円` になる）ので文字としては比べられず、比べているのは**境界の値がどちらに入るか**だけです。コピーの「60cm以下」と「60cmを超え」はどちらも 60cm を小さいほうに入れ、`<=60cm` と `>60cm` も同じことを言います。コピーに境界の語が無いとき（`18 to 20`、`60〜80`）、語が数と別の列にあるとき（保険料額表の「円以上／円未満」）、同じ数をコピーが両側に置いているときは、何も言いません。",
                "A threshold of a row that cites puts its boundary value on the other side from the copy. A threshold is rewritten as it is transcribed (`up to 1,949,000JPY` becomes `<=1949000JPY`) so the text cannot be compared; what is compared is **which of the two bands the boundary value falls in**. The copy's `up to 60cm` and `over 60cm` both put 60cm in the band below, and so do `<=60cm` and `>60cm`. A number the copy bounds with no word (`18 to 20`, `60-80`), with the word in another column (a Japanese insurance premium table puts `円以上`, \"yen or more\", in the heading over its own column), or with words on both sides, is left alone."
            ),
            tr!(
                "出典のコピーを読み直して直してください。`fix.text` は、このセルの境界の側だけを入れ替えた形です。比べる向きは表の形で決まり、コピーが決めることではないので、`fix.text` は `<` と `<=` の入れ替えしか書きません。一つの境界を誤って転記すると、それを分け合う二つの行の両方にこのエラーが出ます。境界が別のところ（後の通知、本文の但し書き）から来たのなら、この行の引用を外し、どこから来たかを行末のコメントに書いてください。",
                "Reread the copy and correct it. `fix.text` is this cell with that one boundary's side swapped and nothing else: the direction is the table's geometry, not the copy's to decide, so only `<` and `<=` are exchanged. One boundary mistranscribed is reported on both of the rows that share it. If the boundary came from somewhere else — a later notice, a proviso in the text — take the citation off this row and say in a comment at the end of it where it came from."
            ),
            X_E119,
            &["E116", "W120", "E105"],
        )
        .with_files(SIZE_DOC)
        .english_with_files(X_E119_EN, SIZE_DOC_EN),
        warn(
            "W122",
            tr!("その `shape` を使っている入力がありません", "No input is projected from that shape"),
            tr!(
                "`shape` を宣言しているのに、`from <その名前>.…` と書いた入力も並びも一つも無いとき。契約は読まれますが、何も確かめていません。",
                "A `shape` is declared and no input or sequence says `from <that name>.…`. The contract is read and holds nothing."
            ),
            tr!(
                "使うか、消してください。読まれているだけの契約は、次に読む人に「ここは契約に縛られている」と思わせます。縛られているのは `from` を書いた入力だけです。",
                "Use it or delete it. A contract that is only read makes the next reader believe this rule is held to it; what is held to it is the inputs that say `from`."
            ),
            X_W122,
            &["E121", "W111"],
        )
        .with_files(SHAPE_DOC)
        .english(X_W122_EN),
        err(
            "E122",
            tr!("契約が通す値を、規則が受け付けません", "The contract lets through a value the rule refuses"),
            tr!(
                "`from` で読む値について、契約の検証は通すのに、入力の宣言が受け付けない値があるとき。比べるのは、数の範囲（Protovalidate の `gte`・`lte` など、JSON Schema の `minimum`・`maximum`）、並びの件数（`min_items`・`max_items`、`minItems`・`maxItems`）、列挙の値（`string.in`、`enum`。順のある列挙の入力に `range` を書いていれば、範囲に入る値と比べます）、JSON Schema の `required` です。`from sum` は、要素のフィールドの値の範囲と要素の数から決まる合計の届く先を、`from max`・`from min` はフィールドの値を比べます。`elements … from` で契約から取る並びは、要素のフィールドを入力と同じに比べ、並びの件数を `count` の上限と、要素の値の合計を `sum` の上限と比べます。proto3 で注釈の無い数のフィールドは、入れ忘れると 0 として届くので、0 を受け付けない入力はここで止まります。`required` の無いメッセージのフィールドと `optional` のフィールドは省略でき、そのとき中の値は規則を通らずに既定値（0、\"\"、0 件）として届きます。`.proto` の文字列から読む日付は、\"\" を通すかどうかを見ます。フィールドをまたぐ条件（CEL の式、`oneof`、JSON Schema の `allOf`・`anyOf`・`oneOf`・`not`・`if`）が一つのフィールドの幅を狭めていれば、それも読みます。JSON Schema の型に null があるのに、入力が省略できないときにも出ます。読めない規則（剰余や文字列の関数を使う CEL など）は、無いものとして扱います。契約を実際より広く読むので、要らないところで出ることはあっても、見逃すことはありません。",
                "A value read with `from` can pass the contract's validation and still be refused by the input's declaration. What is compared: the range of a number (Protovalidate's `gte`, `lte` and the rest; JSON Schema's `minimum` and `maximum`), the length of a collection (`min_items` and `max_items`; `minItems` and `maxItems`), the values of an enum (`string.in`; `enum` — held to the values inside its `range` when an ordered enum input has one), and JSON Schema's `required`. A `from sum` is held by how far its total reaches — the values of the field times how many elements there may be — and a `from max` or `from min` by the values of the field. A sequence taken with `elements … from` has each field of its elements held as an input is, its length held to the cap its `count`s put on it, and its totals to the upper end of each `sum` over it. A proto3 number field with no rule arrives as 0 when it is left unset, so an input that does not take 0 stops here. A message field that is not `required`, and an `optional` field, may be left unset, and the value under it then arrives as its default (0, \"\", no elements) with no rule applied; a date read from a `.proto` string is held to whether \"\" passes. A condition across fields — a CEL expression, a `oneof`, JSON Schema's `allOf`, `anyOf`, `oneOf`, `not` and `if` — is read too where it narrows one field, and a JSON Schema type that has null in it is this code when the input is not optional. A rule that cannot be read, such as CEL with a remainder or a string function, is read as not there: the contract is then read wider than it is, so this may speak where it did not need to, and never stays quiet where it should have spoken."
            ),
            tr!(
                "どちらを直すかは、人が決めることです。その値が来ないはずなら、契約を狭めてください。`fix.text` が、契約に書く注釈そのものです（`narrow_contract`）。来るのなら、規則の範囲を広げるか列挙に値を足して、その値の答えを決めてください。`where` で絞った件数の下限のように契約に書けない前提もあり、そのときは `fix.kind` が `none` です。読む値が無いことがあるなら、入力を `T?` にしてください。値が無いとき、規則はそれを none として読みます。",
                "Which side to change is a person's decision. If the value cannot occur, narrow the contract: `fix.text` is the annotation to write there (`narrow_contract`). If it can, widen the rule's range or add the value to the enum, and decide what it answers. Some preconditions cannot be written in a contract, such as a floor on how many elements a `where` picks out; `fix.kind` is then `none`. When the value read may be missing, make the input `T?`: a missing value is then read as none."
            ),
            X_E122,
            &["W123", "E123", "E121", "E032"],
        )
        .with_files(LINES_DOC)
        .english(X_E122_EN),
        warn(
            "W123",
            tr!("行が、契約の通さない値でしか当たりません", "A row is reached only by values the contract does not let through"),
            tr!(
                "行のセルが `from` で読む入力を試していて、そのセルが受け付ける値を、契約の検証が一つも通さないとき。契約を通ったものしか来ないので、その行に当たるリクエストやメッセージはありません。比べるのは入力そのものの列だけで、そこから導いた値の列は見ません。",
                "A cell of a row tests an input read with `from`, and nothing the cell accepts passes the contract's validation. Only what passed the contract arrives, so no request or message reaches the row. Only a column of the input itself is compared; a column derived from it is not."
            ),
            tr!(
                "契約がこの先も広がらないなら、行を消して、入力の範囲を契約に合わせてください。広がる予定があって残しているのなら、そのままで構いません。CI の `check --diff-base` は、新しく生じたものだけを報告します。",
                "If the contract will not widen, delete the row and bring the input's range in line with the contract. If the row is kept for a widening that is planned, leave it: `check --diff-base` in CI reports only the ones that are new."
            ),
            X_W123,
            &["E122", "W124", "E102", "W111"],
        )
        .with_files(LINES_DOC)
        .english(X_W123_EN),
        err(
            "E123",
            tr!("契約が、規則の `constraint` を破る組み合わせを通します", "The contract lets through a combination the rule's `constraint` refuses"),
            tr!(
                "`constraint` の両側が、同じ `shape` から `from` で読む入力で、契約の検証を通るリクエストのなかに、両方の値が入力の範囲に入っているのに `constraint` を満たさないものがあるとき。契約がフィールドのあいだに置く条件（`.proto` のメッセージの CEL、`oneof`、JSON Schema の組み合わせ）を読んだうえで、それでも破る組み合わせが残るかを確かめ、見つかれば、その値を例に出します。読めない規則は無いものとして扱うので、見逃すことはありません。",
                "Both sides of a `constraint` are inputs read with `from` from the same `shape`, and some request passes the contract's validation with both values inside the inputs' ranges and the `constraint` broken. The conditions the contract places across its fields — CEL on a `.proto` message, a `oneof`, JSON Schema's combinators — are read, and what is asked is whether a breaking combination survives them. When one does, its values are the example. A rule that cannot be read is read as not there, so nothing is missed."
            ),
            tr!(
                "どちらを直すかは、人が決めることです。その組み合わせが来ないはずなら、契約で約束してください。`.proto` なら、`fix.text` がメッセージに書く `(buf.validate.message).cel` です（`narrow_contract`）。JSON Schema には二つのフィールドの値を比べる書き方がないので、`fix.kind` は `none` です。来るのなら、`constraint` を外して、その組み合わせのときの答えを表で決めてください。",
                "Which side to change is a person's decision. If the combination cannot occur, promise it in the contract: for a `.proto`, `fix.text` is the `(buf.validate.message).cel` to write on the message (`narrow_contract`). JSON Schema has no way to compare the values of two fields, so there `fix.kind` is `none`. If it can occur, take the `constraint` off and decide in the tables what the rule answers for it."
            ),
            X_E123,
            &["E122", "W124", "E018"],
        )
        .with_files(QUOTE_PROTO)
        .english(X_E123_EN),
        warn(
            "W124",
            tr!("行が、契約の通さない組み合わせでしか当たりません", "A row is reached only by a combination the contract does not let through"),
            tr!(
                "行のセルが、同じ `shape` から `from` で読む二つ以上の入力を試していて、セルを一つずつ見れば契約の通す値なのに、契約がフィールドのあいだに置く条件のもとでは、その組み合わせが一つも通らないとき。たとえば CEL の `this.min <= this.max` のもとで「最小が 20kg を超え、最大が 10kg 以下」を求める行や、一つの `oneof` の二つのメンバーをどちらも 0 でないとする行です。ほかの列のセルは見ないので、当たると言いすぎることはあっても、当たらないと言いすぎることはありません。",
                "A row's cells test two or more inputs read with `from` from the same `shape`, each cell alone asks for values the contract lets through, and under the conditions the contract places across its fields no combination of them passes: a row asking for a minimum above 20kg and a maximum of at most 10kg under the CEL `this.min <= this.max`, or a row asking for two members of one `oneof` to be both non-zero. The cells on other columns are left out, so this may call a row reachable that is not, never the other way round."
            ),
            tr!(
                "契約がこの先も変わらないなら、行を消してください。変わる予定があって残しているのなら、そのままで構いません。CI の `check --diff-base` は、新しく生じたものだけを報告します。",
                "If the contract will not change, delete the row. If the row is kept for a change that is planned, leave it: `check --diff-base` in CI reports only the ones that are new."
            ),
            X_W124,
            &["W123", "E123", "E102"],
        )
        .with_files(QUOTE_PROTO)
        .english(X_W124_EN),
        err(
            "E124",
            tr!("終わりの状態から出る遷移があります", "A final state has a way out"),
            tr!(
                "`final` に書いた状態から、別の状態へ移る呼び出しがあるとき。終わったはずの案件がまた動くことになります。その状態に着くまでの最短の呼び出しと、出ていく一回が `witness.trace` に付きます。",
                "A call moves a case out of a state the `final` line names: a case that had ended is set going again. The shortest sequence of calls to that state, and the call that leaves it, come with it as `witness.trace`."
            ),
            tr!(
                "その行で状態を留めるか、その状態を `final` から外してください。どちらが正しいかは業務の判断です。",
                "Keep the state where it is on that row, or take it off the `final` line. Which of the two is right is the business's to say."
            ),
            X_E124,
            &["E125", "E126"],
        ),
        err(
            "E125",
            tr!("着いたら終われない状態があります", "A case can reach a state it can never finish from"),
            tr!(
                "`initial` から着ける状態のうち、終わりの状態でなく、そこからどの終わりの状態にも着けないものがあるとき。ワークフローネットの健全性でいう「必ず終われる」が破れている形です。そこに着くまでの最短の呼び出しが付きます。持ち越す数が範囲を出て、次の呼び出しを受けられなくなった案件は、E134 で言います。",
                "A state a case can reach from `initial` is not final, and no final state can be reached from it: the \"option to complete\" of workflow-net soundness is broken. The shortest sequence of calls that gets there comes with it. A case whose carried number has left its range, so that no further call can take it, is E134's."
            ),
            tr!(
                "終わりの状態へ移る行を足すか、そこで終わるのが正しいなら、その状態を `final` に加えてください。",
                "Add a row that moves on to a final state, or, if a case rightly ends there, add the state to `final`."
            ),
            X_E125,
            &["E124", "W125", "E134"],
        ),
        err(
            "E126",
            tr!("`never` の主張が破れています", "A `never` line is broken"),
            tr!(
                "`never A after B` について、`initial` から B を通ったあとで A に着く呼び出しの並びがあるとき。いちばん短いものが付き、そのどの呼び出しも、規則が受け付けて答えを返す入力です。",
                "For `never A after B`, some sequence of calls from `initial` reaches A after it has been in B. The shortest one comes with it, and every call in it is an input the rule takes and answers."
            ),
            tr!(
                "行き先を決める行を直すか、主張が業務として誤りなら `never` の行を消してください。",
                "Correct the rows that decide where a case goes, or drop the `never` line if the claim is wrong as business."
            ),
            X_E126,
            &["E052", "E124"],
        ),
        err(
            "E127",
            tr!("`once` の主張が破れています", "A `once` line is broken"),
            tr!(
                "`once <出力> <セル>` について、一件の案件の中で、出力がセルに当てはまる呼び出しが二回ある並びがあるとき。二重の返金、二重の付与の形です。いちばん短いものが付きます。",
                "For `once <output> <cell>`, one case has two calls whose output the cell accepts: a refund paid twice, a point granted twice. The shortest such sequence comes with it."
            ),
            tr!(
                "二回目に至る並びを断つ行を足すか（返金を済ませた状態から前へ戻らない、など）、宣言が誤りなら `once` の行を消してください。",
                "Add the row that cuts the sequence short before the second call (a case that has been refunded does not go back, say), or drop the `once` line if it is wrong."
            ),
            X_E127,
            &["E053", "E126"],
        ),
        err(
            "E128",
            tr!("ステートマシンの主張を検査できませんでした", "The machine's claims could not be checked"),
            tr!(
                "入力の区画の数が予算（`--budget` を 50 で割った数）を超えたとき、または規則の答えが決まった数の列で区切れないとき。証明できなかった主張を緑にはしないので、警告ではなくエラーです。",
                "The inputs cut into more cells than the budget allows (`--budget` divided by 50), or the rule's answer does not cut into finitely many columns. A claim that was not proven is never green, so this is an error and not a warning."
            ),
            tr!(
                "`--budget` を上げるか（`rulec check` にも `ritsu check` にもあります）、遷移を決める表の列を減らしてください。列の積が効くので、表を一列につないでいくほうが安く済みます。",
                "Raise `--budget` (both `rulec check` and `ritsu check` take it), or give the table that decides the transitions fewer columns. The cost is the product of the columns, so a chain of tables is cheaper than one wide one."
            ),
            X_E128,
            &["E109", "W127"],
        )
        .with_budget(100),
        err(
            "E129",
            tr!("この rulec は koyomi のファイルを読めません", "This rulec reads no koyomi file"),
            tr!(
                "範囲を koyomi の日付からとる規則（`range from koyomi`）を、koyomi をつないでいない rulec で確かめたとき。rulec のクレートのバイナリがそうです。範囲全体で確かめ直すことはせず、確かめられなかったとして終了コード 2 で終わります。",
                "A rule that takes a range from a koyomi date (`range from koyomi`) is checked by a rulec with no koyomi joined: the binary of rulec's own crate. The check does not fall back to every day of the range; the file counts as not checked, and the run exits 2."
            ),
            tr!(
                "`ritsu rulec check <ファイル>` か `ritsu check <ディレクトリ>` で走らせてください。どちらも、koyomi から日の集合を受け取ります。",
                "Run `ritsu rulec check <file>` or `ritsu check <dir>`; both hand the set of days over through koyomi's port."
            ),
            X_E129,
            &["E065", "E130"],
        )
        .with_files(&[("terms.cal", X_TERMS_CAL)]),
        err(
            "E130",
            tr!("koyomi の日付がとる日を読めません", "The days of the koyomi date cannot be read"),
            tr!(
                "`range from koyomi` が指す日付の日を、koyomi が答えないとき。ファイルが無い、koyomi の検査を通らない、その名前の日付が無い、入力の組み合わせが koyomi の確かめる数を超える、途中で計算が止まる入力がある、とる日が一つも無い、のどれかです。注には koyomi のメッセージが出ます。範囲全体で確かめ直すことはしません。",
                "koyomi does not answer for the days of the date `range from koyomi` names: the file is not there, does not pass koyomi's check, or has no date of that name; the inputs come to more combinations than koyomi checks; the computation stops at some input; or the date comes to no day at all. What koyomi says is in the notes. The check does not fall back to every day of the range."
            ),
            tr!(
                "koyomi のファイルを `ritsu koyomi check` で通るように直し、日付の名前を合わせてください。",
                "Make the koyomi file pass `ritsu koyomi check`, and name a date it has."
            ),
            X_E130,
            &["E065", "E129"],
        )
        .with_files(&[("terms.cal", X_TERMS_CAL)]),
        err(
            "E132",
            tr!("割る数が 0 になりえます", "The divisor can be zero"),
            tr!(
                "丸めの関数の引数が値で割る割り算（`half_up(合計 / 件数, 1円)`）で、割る数のとりうる範囲が 0 を含むとき。範囲は、入力の範囲、導出と `define` の範囲、上の表が書く値から区間で求めます。行が自分の表の列をそのまま書くときは、その行のセルがその列に通す値だけを数えます（`| >=1 | 件数 |` は 1 以上の件数だけを書きます）。`constraint` は読みません。範囲の決まらない値を読む割る数も、0 にならないとは言えないので、これに当たります。注には、割る数を 0 にする入力が見つかればそれが出ます。",
                "A rounding function's argument divides by a value (`half_up(total / units, 1JPY)`), and the range of the divisor includes zero. The range is worked out by interval arithmetic from the inputs' ranges, the ranges of the derives and the `define`s, and what the tables above write — where a row writes one of its own table's columns, only the values its cell there lets through (`| >=1 | units |` writes units of one or more). A `constraint` is not read. A divisor that reads a value with no range cannot be shown never to be zero either, and is this too. Where an input that makes the divisor zero is found, the notes name it."
            ),
            tr!(
                "割る数の範囲を 0 を含まないように狭めるか（`range >=1`）、`max(件数, 1)` のように 1 以上にしてから割るか、0 のときに別の値を選ぶ表（`| 0 | 1 |` と `| >=1 | 件数 |` の二行）を置いて、その表の出力で割ってください。`constraint a < b` のもとで `b - a` で割るときも、`max(b - a, 1)` と書きます（その制約のもとでは同じ値です）。0 のときに何を答えるかは業務の決めなので、rulec は決めません。",
                "Narrow the divisor's range so that it leaves zero out (`range >=1`), divide by `max(units, 1)`, or put a table that picks another value when it is zero (the two rows `| 0 | 1 |` and `| >=1 | units |`) and divide by that table's output. Dividing by `b - a` under `constraint a < b`, write `max(b - a, 1)` too: under the constraint it is the same value. What to answer when it is zero is a business decision, and rulec does not make it."
            ),
            X_E132,
            &["E115", "E108", "E019"],
        )
        .english(X_E132_EN),
        err(
            "E139",
            tr!("`isqrt` の引数が負になりえます", "The argument of `isqrt` can be below zero"),
            tr!(
                "`isqrt(x)` の x のとりうる範囲が負の値を含むとき。範囲は E132 と同じく区間で求め、範囲の決まらない値を読む引数もこれに当たります。負の数に平方根は無く、生成コードはその呼び出しで答えを持ちません。注には、引数を負にする入力が見つかればそれが出ます。",
                "The range of x in `isqrt(x)` includes values below zero. The range is worked out by intervals as for E132, and an argument that reads a value with no range is this too. A number below zero has no square root, and the generated code would have no answer on that call. Where an input that makes the argument negative is found, the notes name it."
            ),
            tr!(
                "引数の範囲が 0 以上になるように入力の範囲を狭めるか、`isqrt(max(x, 0))` のように 0 以上にしてから渡してください。冪の切り捨ては掛け算で書きます（`isqrt(n * n * n)` が n の 1.5 乗の切り捨て）。",
                "Narrow the inputs' ranges so that the argument stays at zero or more, or pass `isqrt(max(x, 0))`. A power is written as a product: `isqrt(n * n * n)` is n to the power 1.5, rounded down."
            ),
            X_E139,
            &["E108", "E103"],
        )
        .english(X_E139_EN),
        err(
            "E140",
            tr!("出力の範囲が、出力のとる値を含んでいません", "An output's range does not hold the values it takes"),
            tr!(
                "出力の行に書いた `range` が、その出力のとりうる値を含まないとき。とりうる値は、出力を決めるもの（表の行のセルの値と式、同じ名前の `define`、`result` の式）が入力の範囲と `constraint` のもとでとる値を、丸めの刻みへ外向きに広げたものです。rulec がその値を求められないとき（範囲の無い入力を読む、数でない出力）も、書いた範囲を確かめられないので、これです。出力の範囲は、ほかの言語（kaname、dandori など）が規則の答えを読む範囲なので、狭いと、そこで行う範囲の検査が、規則が実際に返す値を見落とします。",
                "The `range` written on an output's line leaves out values the output can take. What it can take is what decides it — the cells of a table's rows, a `define` of its name, the expression of the `result` — under the inputs' ranges and the `constraint` lines, put out to the grid of its rounding. Where rulec cannot work that out (the output reads an input with no range, or is no number), the range cannot be checked, and is this too. An output's range is the range other languages (kaname, dandori, …) read the rule's answer in, so one too narrow would let their checks of ranges pass over values the rule really returns."
            ),
            tr!(
                "範囲を、文面の示すとりうる値まで広げてください（`fix.text` に `range >=0円 <=1000円` の形で入っています）。確かめられないときは、出力が読む入力に範囲を書くか、出力の範囲を外してください。範囲を書かない出力にも、rulec が求めた範囲が付き、ほかの言語はそれを読みます。",
                "Widen the range to the values the message states (`fix.text` holds it, as `range >=0USDc <=1000USDc`). Where it cannot be checked, give the inputs the output reads a range, or take the output's range off: an output with none written has the range rulec works out, and the other languages read that."
            ),
            X_E140,
            &["E112", "E061", "E103"],
        )
        .english(X_E140_EN)
        .also(tr!("確かめられない範囲", "A range that cannot be checked"), X_E140_UNKNOWN, X_E140_UNKNOWN_EN),
        err(
            "E134",
            tr!("持ち越す数が範囲を出る呼び出しがあります", "A call answers a carried number outside its range"),
            tr!(
                "`initial` から着ける状態からの呼び出しで、持ち越す数の次の値が、その入力の範囲の外になるとき。次の呼び出しにその値を渡せないので、案件はそこから先へ進めません。いちばん短い呼び出しの並びが `witness.trace` に付きます。着けない状態からの呼び出しでだけ範囲を出る行は、W126 で言います。",
                "A call from a state a case can reach from `initial` answers a carried number outside its input's range: the next call cannot be passed it, and the case cannot go on from there. The shortest sequence of calls comes with it as `witness.trace`. A row that leaves the range only from states no case reaches is W126's."
            ),
            tr!(
                "次の値を決める行を直す（上限で止める、別の状態へ移す）か、その値が業務として起きるなら、入力の範囲を広げてください。",
                "Correct the row that decides the next value (stop at the bound, or move on to another state), or widen the input's range if the value does occur in the business."
            ),
            X_E134,
            &["E072", "E135", "W126"],
        ),
        err(
            "E135",
            tr!("`always` の主張が破れています", "An `always` line is broken"),
            tr!(
                "`always` の行が言う状態（`when` の後に書いた状態、書いていなければどの状態でも）に、持ち越す値がセルに当てはまらないまま着く呼び出しの並びがあるとき。いちばん短いものが付き、そのどの呼び出しも、規則が受け付けて答えを返す入力です。持ち越す数が範囲を出たところは E134 が言い、ここでは見ません。",
                "Some sequence of calls reaches a state the `always` line speaks of (those after `when`, or any state when there are none) with a carried value its cell does not accept. The shortest one comes with it, and every call in it is an input the rule takes and answers. A place where a carried number has left its range is E134's, and is not looked at here."
            ),
            tr!(
                "行き先や次の値を決める行を直すか、主張が業務として誤りなら `always` の行を直してください。",
                "Correct the rows that decide where a case goes and what it carries on, or the `always` line if the claim is wrong as business."
            ),
            X_E135,
            &["E072", "E126", "E134"],
        ),
        err(
            "E136",
            tr!("前提が循環しています", "The prerequisites go round in a cycle"),
            tr!(
                "値から要るものへ、そのまた要るものへとたどると、元の値に戻るとき。`or` の中の値もたどります。互いにたどれる値の組ごとに一つ出て、組の最初の値から戻るまでの輪を一つと、組の値を挙げます。輪の上の値は、どれもほかの値を先に持っていないと手に入らないので、輪のどこからも始められません。`or` の別の値から輪に入れる場合も、前提の読み違いのもとなので、エラーにします。",
                "Following what a value needs, and what that needs, leads back to the value. The values inside an `or` are followed too. One is given for each set of values that reach one another, with one way round from the first value of the set back to it, and the values of the set. Each value on a cycle waits for another on it, so the cycle cannot be entered anywhere; where another value of an `or` does lead in, a cycle is still where prerequisites are misread, and it is an error."
            ),
            tr!("輪のどこかの前提を外してください。", "Drop one of the prerequisites on the cycle."),
            X_E136,
            &["E071", "E137"],
        ),
        err(
            "E137",
            tr!("`start` からたどり着けない値があります", "A value cannot be reached from `start`"),
            tr!(
                "`start` の値から始めて、要るもののどのグループにも持っている値がある値を足していっても、足されない値があるとき。循環が無ければ、足されないのは手に入らない値（`requires nothing`）を待つ値で、手に入らない値だけのグループを持つ値ごとに一つ出ます。その値を待つほかの値は注記に挙げます。循環は E136 が言い、ここでは見ません。手に入らないと書いた値そのものは言いません。",
                "Starting from the values of `start` and adding every value each group of whose needs has a value already held, some value is never added. With no cycle, what is never added waits on a value no case can take (`requires nothing`): one is given for each value with a group of such values alone, and the values that wait on it are listed in a note. A cycle is E136's and is not looked at here. A value written as one no case can take is not said: it says so itself."
            ),
            tr!(
                "手に入らない前提を外すか、その値も手に入らないなら `<値> requires nothing \"<理由>\"` と書いてください。",
                "Drop the prerequisite no case can take, or, if the value cannot be had either, write `<value> requires nothing \"<reason>\"`."
            ),
            X_E137,
            &["E071", "E136"],
        ),
        err(
            "E138",
            tr!("`ordered by` の順で、前提が値より後に来ます", "A prerequisite comes after the value in the order of `ordered by`"),
            tr!(
                "`ordered by` が名前を挙げた表で、値の答えが、その値に要るもの（`or` の中の値も）の答えより前にあるとき。手に入らない値への前提と、循環の上の前提は、E137 と E136 が言うので、ここでは見ません。",
                "In the table `ordered by` names, a value's answer comes before the answer of something it needs (a value inside an `or` too). A prerequisite no case can take, and one on a cycle, are E137's and E136's, and are not looked at here."
            ),
            tr!("表の答えか、前提のどちらかを直してください。", "Correct the table's answer, or the prerequisite."),
            X_E138,
            &["E071", "E136"],
        ),
        warn(
            "W125",
            tr!("どの手順でも着かない状態があります", "No sequence of calls reaches a state"),
            tr!(
                "持ち越す状態の列挙に、`initial` から始まる呼び出しの並びでは着かない値があるとき。",
                "The enum of the carried state has a value that no sequence of calls from `initial` reaches."
            ),
            tr!(
                "その状態へ移る遷移を足すか、要らなければ列挙から消してください。ほかの版から移ってくる案件のために残しているのなら、このままで構いません（移行は `diff` が見ます）。",
                "Add the transition into it, or drop it from the enum. If it is kept for cases moved over from another version, leave it: `diff` looks at the move."
            ),
            X_W125,
            &["W126", "E125"],
        ),
        warn(
            "W126",
            tr!("案件が着ける状態からは使われない遷移があります", "A transition is never taken from a state a case can reach"),
            tr!(
                "遷移を決める表の行が、`initial` から着けない状態のときにしか当てはまらないとき。W125 と一緒に出ます。状態には着けても、その行が要る `held` の入力の値や、持ち越す値（`carry` の二行目から）では着かないときも、この警告です（持ち越す数が範囲を出る行が、着けない値のときにしか当たらない場合もここに入ります）。",
                "A row of the table that decides the transitions applies only in states no sequence of calls from `initial` reaches. It comes with W125. A row whose state a case does reach, but never with the value of a `held` input or of a carried value (a `carry` line after the first) the row needs, is this warning too — among them a row that would answer a carried number outside its range, from values no case holds."
            ),
            tr!(
                "着けない状態へ移る遷移を足すか、行を消してください。",
                "Add the transition into the state it needs, or drop the row."
            ),
            X_W126,
            &["W125", "E102"],
        ),
        warn(
            "W127",
            tr!("ステートマシンの主張を決めきれませんでした", "A claim of the machine could not be settled"),
            tr!(
                "主張が、答えを出す入力を作れず、起こらないとも示せなかった区画に左右されるとき。導出どうしが入力を共有していて、解が有理数にしか無い形がここに残ります（W114 と同じ場所です）。数を持ち越す機械（`carry` の二行目から）では、ほかに三つあります。数の値を一つずつ調べると予算を超えるので区間にまとめて調べ、区間の上で破れる並びを、本当に起きる呼び出しでは作り直せなかったとき。区間にまとめたので、どの状態からも終わりに着けるかを確かめていないとき。持ち越す数の次の値が区画の中でほかの入力とともに動き、あいだの値を一つずつは試していないとき。決めきれなかったことを、成り立つとは言いません。",
                "A claim turns on a cell for which no input was built and none was shown impossible. Derived values that share an input, with a solution only among the rationals, are what is left here — the place W114 is about. A machine that carries numbers (a `carry` line after the first) leaves three more: the numbers were walked in intervals, since every value one by one is past the budget, and a sequence that breaks the claim over the intervals could not be made again from calls that really happen; walked in intervals, that every state a case reaches can still get to a final one is not followed; or a carried number's next value moves with another input inside a cell, and the values between the ones the calls seen answered were not each tried. What was not settled is not said to hold."
            ),
            tr!(
                "その区画に当たる入力が本当に無いなら、条件を整数で書き直してください（`倍 >= 5円` を `a >= 3円` にする、など）。あるなら、その入力を例か手順の例に書いてください。区間にまとめたためなら、`--budget` を上げるか持ち越す数の範囲を狭めると、値を一つずつ調べて決めます。",
                "If no input really falls in the cell, write the condition over whole numbers (`a >= 3JPY` for `twice >= 5JPY`). If one does, write it down as an example or a scenario. Where the carried numbers were walked in intervals, raising `--budget` or narrowing their ranges lets every value be walked one by one, and the claim settled."
            ),
            X_W127,
            &["W114", "E128"],
        )
        .english(X_W127_EN),
        warn(
            "W128",
            tr!("行に当たる入力を見つけられず、当たらないことも示せませんでした", "No input was found that reaches a row, and none was shown not to"),
            tr!(
                "計算した列（導出、`define`）にセルを書いた行で、その行に当たる入力が、規則の例にも、ベクタを作るときの探し方でも、入力の候補の値の組み合わせ（4,096 通りまで）にも、その行を除いたときに完全性の検査が見つける穴を作る入力にも見つからず、どの入力も当たらないことも示せなかったとき。そういう行の点は入力ではなく、そこに届く入力があるかは式どうしの関係で決まります（整数でしか届かない、整数では届かない、など）。届くと決まっていない行を、黙って通しません。入力と列挙だけの行は、点がそのまま入力なので、この探しはしません。",
                "A row that writes a cell on a computed column (a derive, a `define`): no input reaching it was found among the rule's examples, by the search the vectors are made with, among the combinations of the inputs' candidate values (up to 4,096), or as the input behind the gap the completeness check finds with the row taken out, and it could not be shown that no input does. The point of such a row is not an input, and whether one reaches it turns on how the expressions relate (a value only whole numbers reach, or one they never do). A row nothing decided is not passed in silence. A row of inputs and enums alone needs no search: its point is an input."
            ),
            tr!(
                "その行に当たる入力が本当にあるか、式と行の条件を見比べてください。あるなら、その入力を例に書くと、この警告は消えます。無いなら、行を削除してください。削除して完全性の欠落（E101）が出るなら、その組み合わせが起きないことも rulec は示せていないので、行は残してかまいません。",
                "Look at the expressions and the row's conditions together. If some input reaches the row, write it as an example and the warning goes; if none does, delete the row. If deleting it brings a completeness gap (E101), rulec cannot show that the combination never comes either, and the row can stay."
            ),
            X_W128,
            &["E101", "E102", "W114", "W127"],
        )
        .english(X_W128_EN),
        warn(
            "W130",
            tr!("ほかの前提からたどれる前提があります", "A prerequisite is already needed through another"),
            tr!(
                "値の前提の一つが、同じ値のほかの前提が要るもの（そのまた要るものも）の中にあるとき。書いても書かなくても、その値を取れるときは同じです。`or` の中の値は、どちらの側でも見ません。",
                "One of a value's prerequisites is among what another of its prerequisites needs, or what that needs in turn: written or not, it leaves the value to be taken at the same times. The values inside an `or` are left out on both sides."
            ),
            tr!(
                "その前提を消してください。前提を直すときに書き直す所が一つ減ります。",
                "Drop it: one place less to correct when the prerequisites change."
            ),
            X_W130,
            &["E136"],
        ),
        warn(
            "W105",
            tr!("要確認の隠れ: 先の行が後の行の一部を隠しています", "Shadowing that needs review: an earlier row hides part of a later one"),
            tr!(
                "`policy first` の表で、一部だけ重なっていて出力が違う行の対があるとき。階段状の隠れと、答えが同じ隠れは件数の注記に畳まれ、ここに一覧されるのは要確認の対だけです。重なりがどの入力も届かない点だけにあるときは、E105 と同じく報告しません。",
                "In a `policy first` table, two rows partially intersect and disagree on the output. Structural shadowing (the staircase) and equivalent shadowing are folded into a count line; only the pairs that need review are listed. A pair that meets only where no input reaches is not reported, as under E105."
            ),
            tr!(
                "意図どおりならこのままで構いません（CI の `check --diff-base` は新たに生じた対だけを報告します）。後の行を優先したいなら、その行を先の行より上へ移してください。すべての対を見るには `--show-shadow` を付けてください。",
                "If it is intended, leave it: `check --diff-base` in CI reports only newly created pairs. To let the later row win, move it above the earlier one. `--show-shadow` lists every pair."
            ),
            X_W105,
            &["E105", "W110", "E102"],
        )
        .english(X_W105_EN),
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
                "入力・導出・グループ・列挙の値が、どの表のどのセルにも現れないとき。書き忘れのしるしであることも、意図した契約であることもあります。`import` で持ち込んだ型の値は対象外です。順のある列挙の値のうち、その列挙を型に持つ名前がどれも入力で、どの入力の `range` もその値を外しているものも対象外です。その値を持ってくる呼び出しは無いからです。入力をそのまま渡すセル（`| false | requested |`）が答える値も、その列挙をどの表も列で確かめていなければ対象外です。その値の行を持てる表が無いからです。",
                "An input, a derived value, a group or an enum value appears in no cell of any table. It can be the symptom of a forgotten column, or a legitimate contract. Values of an imported type are not checked this way, nor is a value of an ordered enum when every name of that type is an input whose `range` leaves the value out: no call brings it. Nor is a value a cell that hands an input on (`| false | requested |`) answers with, when no table tests the enum in a column: no table could give it a row of its own."
            ),
            tr!(
                "入力が範囲の入口検査としてだけ効いているなら、宣言に `contract_only` を付けて黙らせてください。列挙の値が既定行に落ちるのが正しいなら、値の宣言に `default` を付けてください。どちらでもないなら、その列を表に足すか、宣言を消してください。",
                "If the input serves only as a range check at the entry, mark the declaration `contract_only`. If an enum value is meant to fall through to the default row, mark the value `default`. Otherwise either add the column to a table, or delete the declaration."
            ),
            X_W111,
            &["E101", "E012"],
        ),
        warn(
            "W116",
            tr!("どの例も使っていない `sequence` です", "No example uses this sequence"),
            tr!(
                "`sequence` を書いたのに、どの例もその名前を書いていないとき。並びは、例が使ったときにだけ走るので、走っていない並びです。",
                "A `sequence` is written and no example names it. A sequence runs only when an example names it, so this one never runs."
            ),
            tr!(
                "その並びをたどる例を足すか、並びのほうを消してください。書いたのに使っていないのは、たいてい例を書き忘れた跡です。",
                "Add the example that walks it, or drop the sequence. Written and unused is usually the trace of an example left unwritten."
            ),
            X_W116,
            &["E027", "W111"],
        )
        .english(X_W116_EN),
        warn(
            "W119",
            tr!("ハッシュを書いた箇所が引用されていません", "A pinned fragment is not cited"),
            tr!(
                "`source` の下にハッシュの行があるのに、その箇所を引用する `@` が規則のどこにも無いとき。引用を消したあとの残りです。",
                "A pin line sits under a `source` line, but no `@` in the rule cites that fragment. It is what remains after a citation was removed."
            ),
            tr!("その行を消してください。`rulec source pin` でも消せます。", "Remove the pin line; `rulec source pin` does."),
            X_W119,
            &["E037"],
        )
        .with_files(ARTICLE_1),
        warn(
            "W120",
            tr!("出典のコピーにある値を、どの行も使っていません", "The copy states a value no row uses"),
            tr!(
                "表が `@出典 表1` で丸ごと引いている出典のコピーに、数だけでできたセルがあって、その値をどの行も使っていないとき。行を一本落としても、完全性の検査には出ません。落ちた行の入力は、残った行のどれかに当てはまってしまうからです。数だけのセルしか見ないので、`2026年4月1日改定` のような文は金額として数えません。`<=3kg` の一行でコピーの `1kg`・`2kg`・`3kg` をまとめて転記した場合も出ません。",
                "A cell of the copy a table cites whole with `@source table1` is nothing but a number, and no row uses that value. A dropped row does not show up in the completeness check: its inputs fall into one of the rows that remain. Only cells that are nothing but a number are asked about, so `revised 1 April 2026` is not counted as an amount, and a row that merges what the copy lists — `<=3kg` over its `1kg`, `2kg` and `3kg` — accounts for all of them."
            ),
            tr!(
                "出典のコピーと見比べて、落とした行がないか確かめてください。改定で行が増えたのなら、その行をここに転記してください。引用した表のうち一部だけを転記したのなら（発地ごとの運賃表から、一つの発地だけを転記したときなど）、引用を `table` の行から、転記した行それぞれの末尾へ移してください。行の引用は「この行はここから来た」としか言わないので、転記していない残りの行は問われなくなります。金額の突き合わせ（E116）は残ります。",
                "Compare the table with the copy and check that no row was left out; if a revision added a row, transcribe it. If the table transcribes only part of the fragment — one origin of a tariff sheet that lists several — move the citation from the `table` line onto the end of each row that came from it. A row's citation says only where that row came from, so the rest goes unasked while the amounts are still held to the copy (E116)."
            ),
            X_W120,
            &["E116", "W119"],
        )
        .with_files(TARIFF_DOC)
        .english_with_files(X_W120_EN, TARIFF_DOC_EN),
        warn(
            "W118",
            tr!("準用した表の行が、この規則ではどれも当たりません", "No row of an applied table is reached in this apply"),
            tr!(
                "準用した表か節の**全行**が、この規則では当たらないとき。読み替えた値がその表の条件に届かないか、この規則のほかの定義（`overrides 準用名:表` で優先する節など）が全部先に取っています。一部の行が当たらないだけなら何も言いません。元の規則の表はこの規則より広い範囲に書かれているのが普通で、そうした行は `doc` が「この準用では当たらない行」として挙げます。",
                "**Every** row of an applied table or clause is unreachable in this rule: what is bound never reaches its conditions, or other definitions of this rule (a clause with `overrides apply:table`, say) take precedence over all of it. Rows unreachable one by one draw no word: a callee's table is usually written for a wider range than this rule's, and `doc` lists those rows as unused by this apply."
            ),

            tr!(
                "その表がこの準用に要らないなら `except <表>` で外してください。要るはずなら、読み替えか `with` の値の対応を見直してください。",
                "If this apply does not need the table, leave it out with `except <table>`. If it should be used, look at the bindings and the `with` mapping."
            ),
            X_W118,
            &["E102", "W117"],
        )
        .with_files(CALLEE)
        .english_with_files(X_W118_EN, CALLEE_EN),
        warn(
            "W117",
            tr!("効かない例外です", "An exception with no effect"),
            tr!(
                "`overrides` で優先すると書いた相手の行と、この表の行が一つも交わらないとき。優先は、両方に当てはまる入力があるときにどちらが勝つかを決めるものなので、交わらなければ何も決めていません。たいていは、ただし書が本文の一部を切り出す形に転記されていない、という転記の誤りです。",
                "No row of this table meets a row of what its `overrides` line names. Precedence decides which wins when an input matches both, so with no meeting rows the line decides nothing. It is the usual sign of a proviso transcribed so that it no longer carves out part of the main rule."
            ),
            tr!(
                "原文と見比べて、この表の行の条件か相手の行の条件を直してください。本当に交わらないなら、`overrides` の行を消してください。",
                "Compare with the source and fix the conditions of this table's rows or of the target's. If they really never meet, drop the `overrides` line."
            ),
            X_W117,
            &["E035", "E105"],
        )
        .english(X_W117_EN),
        warn(
            "W121",
            tr!("生成先の言語で、識別子がぶつかるか使えません", "An identifier collides, or cannot be used, in a target language"),
            tr!(
                "ASCII の別名が、生成先のどれかの予約語か、その言語がすでに使っている名前と同じとき。別名はそのまま関数・引数・型・メンバの名前になります。規則の別名は、モジュールやパッケージの名前にもなります。そこで標準ライブラリと同じ名前だと、生成したモジュールがぶつかります。また、規則と、規則が宣言する列挙の型の名前が同じになるとき。Go は規則の関数と列挙の型を一つのパッケージに並べ、Java は列挙を規則のクラスの中に置くので、どちらもコンパイルできません（`enums` のファイルから読む列挙は、そのファイルのパッケージやクラスの名前で書くので、ぶつかりません）。別名の無い名前が予約語のときも出ます。そのときは生成コードがどの言語でも名前の後ろに `_` を付けて書き（`class_`）、それがこの規則のほかの名前の識別子でもあれば、さらに番号を付けます（`class_2`）。書いた二つの名前の識別子（別名、無ければ名前）が違っても、どれかの生成先の綴りで一つになるときにも出ます。綴りは、値を書く lowerCamelCase、構造体のフィールドの PascalCase、`.proto` のフィールドの JSON の名前、列挙の値の大文字、予約語や言語が使う変数の名前に付ける `_`、大文字と小文字を区別しない列の名前（SQLite）、名前の先頭の 63 バイト（PostgreSQL）です。識別子をその生成先が使えないときも出ます。大文字で始まる名前を定数として読む言語、`_` で始まるフィールドを受け付けない `NamedTuple`、63 バイトより長い列の名前を切る PostgreSQL、`_` だけの名前を変数にもフィールドにもできない言語、列の名前に `true` と `false` を付けない SQLite、ASCII でない名前を取らない `.proto` です。書いた名前が、生成したコードが自分で使う名前と同じになるときにも出ます。当てはまった行と答えと要素の型、誤りの型、丸めなどの補助関数、局所変数、組み込みの名前、`.proto` のメッセージとフィールド（`DecideResponse` の `trace`）、列挙の値の `<型>_UNSPECIFIED`、Connect のサービスが読み込む名前、言語のコマンドが取ってあるパッケージの名前、プログラムが始まるファイルの名前です。どの生成先でそうなるかは、警告の注記が言います。どの言語でも一つに綴る二つの名前（二つの列挙の型など）は、E034 で止まります。",
                "An ASCII alias is a keyword of one of the targets, or a name that language already uses. An alias becomes a function, a parameter, a type or a member there. The rule's alias also names a module or a package, and the standard library's own names are held against it there. Or a rule and an enum it declares take one type name: Go declares the rule's function and the enum's type in one package, and Java nests the enum in the class the rule is, so neither compiles (an enum read from an `enums` file is written by that file's package or class, and does not clash). It is raised too for a name with no alias that is a keyword: the generated code of every language then writes the name with `_` after it (`class_`), and with a number after that where `class_` is the identifier of another name of the rule (`class_2`). It is raised as well where two names the author wrote, whose identifiers (the alias, else the name) differ, are one identifier in the spelling of some target: the lowerCamelCase values are written in, the PascalCase of a struct's fields, the JSON name of a field of the `.proto`, the upper case of an enum's values, the `_` put after a keyword or after the name of a variable a language keeps, a column's name read with case set aside (SQLite), and the first 63 bytes of a name (PostgreSQL). And where a target cannot take an identifier: a language that reads a name starting with a capital as a constant, a `NamedTuple`, which takes no field starting with `_`, PostgreSQL, which cuts a column's name longer than 63 bytes, the languages in which a name of `_` alone is neither a variable nor a field, SQLite, which gives a column no name of `true` or `false`, and the `.proto`, which takes no name that is not in ASCII. And where a name the author wrote is one the generated code uses itself: the types of the rows that matched, of the answer and of one element, the errors, the helpers that round and the rest, its locals, the names the language has built in, the messages and the fields of the `.proto` (the `trace` of `DecideResponse`), the `<TYPE>_UNSPECIFIED` of an enum's values, the names the service of Connect imports, the package names a language's command keeps, and the name of the file a program starts in. The warning's notes say which targets. Two names every language spells as one (the types of two enums, say) stop the check with E034."
            ),
            tr!(
                "予約語なら、その言語の生成コードはコンパイルが通りません（`type` を入力の別名にすると Rust が落ちます）。すでにある名前なら、規則の関数や列挙の型がそれを隠します（`sum` を規則の別名にすると Python の組み込みが隠れます）。モジュールの名前なら、`time` を規則の別名にすると、生成した `time` のモジュールが標準ライブラリの `time` とぶつかります。どの生成先でどうぶつかるかは、警告の注記に出ます。引数やローカル変数の名前は、その本体の外までは隠しません。そのため、引数やローカル変数の名前でこの警告が出るのは、予約語のときだけです。二つの名前がある生成先で一つの識別子になるなら、その生成先を使うときは、どちらかに別名を付けてください。生成先が使えない識別子や、生成したコードが自分で使う名前なら、使える別名を付けてください（大文字で始まらない、`_` で始まらない、63 バイト以下の、生成したコードが使わない別名）。使わない生成先なら、このままで構いません。",
                "A keyword means the generated code for that language does not compile (`type` as an input's alias breaks Rust). A name that is taken means the rule's function or an enum's type hides it (`sum` as the rule's alias hides Python's builtin). A module's name means, with `time` as the rule's alias, that the module generated as `time` collides with the standard library's `time`; the warning's notes say where and how. A parameter or a local shadows nothing outside its own body, so there the warning is raised only for a keyword. Of two names that are one identifier in a target you generate, give one an alias; to an identifier a target cannot take, or a name its generated code uses itself, give an alias it takes (one that starts with neither a capital nor `_`, is 63 bytes or fewer, and is no name the generated code uses). For a target you do not generate, leave it."
            ),
            X_W121,
            &["E009", "E011", "E034"],
        )
        .english(X_W121_EN)
        .also(tr!("Swift と Java が同じに綴る二つの名前", "Two names Swift and Java spell alike"), X_W121_CAMEL, X_W121_CAMEL_EN)
        .also(tr!("SQLite が一つの列として読む二つの名前", "Two names SQLite reads as one column"), X_W121_CASE, X_W121_CASE_EN)
        .also(tr!("PostgreSQL が一つの名前に切る二つの長い名前", "Two long names PostgreSQL cuts to one"), X_W121_LONG, X_W121_LONG_EN)
        .also(tr!("Ruby が定数として読む名前", "A name Ruby reads as a constant"), X_W121_RUBY, X_W121_RUBY_EN)
        .also(tr!("生成したコードが自分で使う名前", "A name the generated code uses itself"), X_W121_OWN, X_W121_OWN_EN)
        .also(tr!("`_` だけの名前", "A name of `_` alone"), X_W121_BLANK, X_W121_BLANK_EN),
        warn(
            "W115",
            tr!("どの要素もこの判定にはなりません", "No element can land on this verdict"),
            tr!(
                "`fold` にその判定の行き先があるのに、表のどの行もその判定を出さないとき。E024 の裏返しで、こちらは穴ではなく、どの要素も届かない行き先です。たいていは書き間違いではなく、表のほうが変わった跡です。",
                "A `fold` has an arm for a verdict no row produces. It is the other side of E024: not a hole but an arm nothing reaches, and more often the trace of a table that changed than of an arm written by mistake."
            ),
            tr!(
                "表の行を見直すか、その行き先を消してください。どちらが正しいかは表のほうを読まないと決まりません。",
                "Look again at the table's rows, or drop the arm. Which of the two is right is decided by reading the table, not this message."
            ),
            X_W115,
            &["E024"],
        )
        .english(X_W115_EN),
        warn(
            "W114",
            tr!("未確認の重なり: 両方に当てはまる入力が有り得ます", "Unconfirmed overlap: an input may match both rows"),
            tr!(
                "`policy unique` の表で二行が重なりうるが、それを実際に起こす入力を構成できず、実現不能の証明もできなかったとき。導出が入力を共有する形と、真偽の `define` の中の閾値は、消去が決めるようになったのでここには落ちてきません。残るのは、消去が有理数の上で解いているために決まらない形です。下の最小の再現の `倍` は必ず偶数なので `5円` ちょうどにはなりませんが、有理数には `2.5円` があります。不等式の数が上限（400 本）を超えた場合と、単位をまたぐ場合も同じです。どれも「証明できなかった」という意味で、「起こりうる」という意味ではありません。上流の表がその値を出す行の条件と導出の組み合わせでしか重ならない対は、検査がその組み合わせを読んで重なりから外すので、ここには来ません。",
                "Two rows of a `policy unique` table may overlap, but no input producing that was constructed and infeasibility was not proven either. Derived values sharing an input and the thresholds inside a boolean `define` no longer fall here: the elimination decides them. What is left is what it cannot decide because it works over the rationals — `twice` above is always even and never exactly `5JPY`, but `2.5JPY` is a rational. A system past the cap of 400 inequalities and one that spans two units are the same: not proven, which is not the same as possible. A pair that meets only where the rows of a table above leave a derive no values does not fall here either: the check reads that and drops it."
            ),
            tr!(
                "その条件を同時に満たす入力が存在するなら、行を直してください（出力が違うので、当てはまれば矛盾です）。存在しないなら、このままで構いません。生成コードには、万一その条件に当てはまる入力が来たとき黙って先の行を選ばずエラーを返すガードが入ります。",
                "If an order satisfying both conditions can exist, fix the rows: the outputs differ, so a match is a contradiction. If none can exist, leave it — the generated code carries a guard that returns an error rather than silently picking the earlier row."
            ),
            X_W114,
            &["E105", "W105", "E109"],
        )
        .english(X_W114_EN),
        warn(
            "W901",
            tr!("鍵がファイルに書かれています", "A key is written in the file"),
            tr!(
                "規則のどこか（文字列でもコメントでも）に、鍵の形の値があるとき。調べる鍵は、AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵で、どれもプロバイダーが接頭辞や形を決めているものです。ritsu のどの言語も同じ決まりで調べます。診断には鍵の種類と、接頭辞と、長さだけを出し、鍵そのものも、その行も出しません。",
                "Somewhere in the rule, in a string or a comment alike, there is a value in the shape of a key: an AWS access key ID, a key or token of GitHub, Slack, Stripe, OpenAI, Anthropic or Google, a Slack incoming webhook URL, or a PEM private key, each a shape its provider fixes. Every language of ritsu looks for them the same way. The diagnostic gives the kind of key, its prefix and its length, and never the key nor its line."
            ),
            tr!(
                "鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。本物の鍵なら、まずプロバイダーで無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。",
                "Keep the key where the code runs (an environment variable, the platform's connection or secret store) and read it from there. If it is real, revoke it with its provider first: taking it out of the file leaves it in the history of the repository. If it is a value for tests, write `ritsu: test secret` in a comment on the same line."
            ),
            X_W901,
            &[],
        )
        .english(X_W901_EN),
        err(
            "E074",
            tr!(
                "`from sum`・`from max`・`from min` か `elements … from` の形が違います",
                "A `from sum`, `from max` or `from min`, or an `elements … from`, is misshapen"
            ),
            tr!(
                "`from sum|max|min` に `of <要素のフィールド>` が無いとき、`from max`・`from min` に `empty <値>` が無いとき、`empty` の値が入力の型の値でないか、入力の範囲の外にあるとき、`of` や `empty` をそれを取らない `from` に書いたとき（`empty` が要るのは `max` と `min` だけです。合計なら 0、件数なら 0 件に決まっています）。また、`elements <名前>` の後ろに `from <shape の名前>.<並び>` 以外のものを書いたとき、契約から取る並びのフィールドに `from` が無いとき、契約から取らない並びのフィールドに `from` があるとき、要素のフィールドの `from` が要素の中のパスでないとき。",
                "A `from sum`, `from max` or `from min` has no `of <field of an element>`; a `from max` or `from min` has no `empty <value>`, or its `empty` value is not a value of the input's type, or lies outside the input's range; or `of` or `empty` is written on a `from` that takes none (only `max` and `min` need `empty`: a sum of none is 0, and a count of none is 0). Also: something other than `from <shape>.<collection>` follows the name of an `elements` line; a field of a sequence taken from a contract has no `from`; a field of a sequence not taken from one has one; or the `from` of a field of an element is not a path inside the element."
            ),
            tr!(
                "形を直してください。`from max 注文.lines of amount empty 0円` のように書きます。`empty` は見る要素が一つも無いときの入力の値で、何にするかは業務の判断です。並びを契約から取るなら `elements 明細 from 注文.lines` と書き、フィールドごとに、要素の中のどこから読むかを `from qty` のように書いてください。並びを呼び出し側が組み立てて渡すなら、フィールドの `from` を消してください。",
                "Correct the shape: `from max order.lines of amount empty 0JPY`. `empty` is the input's value when no element is looked at, and what it is is a business decision. To take a sequence from the contract, write `elements lines from order.lines`, and on each field the path it is read from in an element, such as `from qty`. If the caller builds the sequence and passes it, take the `from` off its fields."
            ),
            X_E074,
            &["E120", "E121", "E122", "E022"],
        )
        .with_files(AMOUNTS_DOC)
        .english(X_E074_EN),
        err(
            "E070",
            tr!("乱数の入力か確率の主張の形が違います", "A random input or a claim of `chances` is not shaped right"),
            tr!(
                "`random` を付けた入力が引けないとき。引けないのは、範囲の両端の無い数、値の無いことがある入力（`?`）、日付と文字列、`from` で呼ぶ側のオブジェクトから取り出す入力（`from sum` などで取り出す値も、呼ぶ側の値で、引いた値ではありません。`random` を `from` の前に書いても後ろに書いても同じです）、machine が持ち越す入力です。`random` を出力か要素のフィールドに書いたとき。`chances` の形が違うとき：`chances` の行に何かが続く、下に表が無い、見出しに `->` が無い、最後の列が `chance` でない、出力の列が無い、行が無い。乱数でない入力に列の無いものがある、左に乱数の入力や入力でない名前を書いた、右に出力でない名前を書いた、並びをたどる規則で並びの列に `sequence` の名前でないものを書いた、確率のセルが 0 から 1 の有理数・率・その比べとして読めないとき。乱数の入力が無いのに `chances` があるときもこれです。",
                "An input marked `random` cannot be drawn: a number without both ends of its range, an input that may be absent (`?`), a date or a string, an input read out of the caller's object with `from` (what `from sum` and the like read is the caller's value, not a draw, whether `random` is written before the `from` or after it), or an input a machine carries. Or `random` is written on an output or on a field of an element. Or `chances` is misshapen: something follows `chances` on its line, there is no table under it, the header has no `->`, its last column is not `chance`, it names no output, or there are no rows; an input that is not random has no column, a random input or a name that is not an input is on the left, a name that is not an output is on the right, a sequence's column in a rule that walks holds something other than the name of a `sequence`, or a chance cell cannot be read as a rational, a rate or comparisons with one, between 0 and 1. A `chances` table in a rule with no random input is this too."
            ),
            tr!(
                "引く数には `range >=0 <=13` のように両端のある範囲を書き、数か列挙か真偽を引いてください。`chances` の見出しは、乱数でない入力の全部の列、`->`、主張に要る出力の列、最後に `chance` です。どの値でもよい列には `-` を書き、確率は `5/14`、`35%`、`<=1/100` のように書きます。",
                "Give a number that is drawn a range with both ends, as in `range >=0 <=13`, and draw a number, an enum or a truth value. The header of `chances` names every input that is not random, then `->`, the outputs the claims are about, and `chance` last. A column that takes any value gets `-`, and a chance is written like `5/14`, `35%` or `<=1/100`."
            ),
            X_E070,
            &["E073", "E133", "E108"],
        ),
        err(
            "E073",
            tr!("重みで選ぶ畳み込みの形が違います", "A pick by weight is not shaped right"),
            tr!(
                "`take_weighted` に `by` が無いとき、`draw` の行が二つあるとき、`take_weighted` の無い畳み込みに `draw` の行があるとき。重みで選ぶ畳み込みが一つの規則に二つあるとき、その腕に `next` と `take_weighted` のほかの腕があるとき、重みが number でないか負になりうるとき、`draw` の行が無いとき、`draw` が指す値が、`random` を付けて範囲が 0 から始まる number の入力でないとき。",
                "A `take_weighted` has no `by`; there are two `draw` lines; or a fold with no `take_weighted` has a `draw` line. Or a rule has two folds that pick by weight; such a fold has an arm other than `next` and `take_weighted`; a weight is not a number, or can be negative; there is no `draw` line; or what `draw` names is not an input marked `random` that is a number whose range starts at 0."
            ),
            tr!(
                "腕 `take_weighted <値> by <重み>` と、行 `draw -> <乱数の入力>` を一行書き、指す入力は `range >=0 <=<上限>` の `random` な number にしてください。重みは 0 以上の整数です。選ぶのが二回要るなら、規則を二つに分けます。",
                "Write the arm `take_weighted <value> by <weight>` and one line `draw -> <random input>`, and make the input it names a `random` number with `range >=0 <=<top>`. A weight is a whole number, 0 or more. Split the rule in two if it needs two picks."
            ),
            X_E073,
            &["E021", "E070"],
        ),
        err(
            "E133",
            tr!("確率の主張が成り立ちません", "A claim of `chances` does not hold"),
            tr!(
                "`chances` の行の確率が、行の入力のセルに当てはまる乱数でない入力のある組み合わせで、乱数の入力を一様に引いたときの確率と違うとき（比べる形なら、それを満たさないとき）。その組み合わせと、そこでの確率が付きます。確率は、乱数の入力の点を数えた正確な有理数です。",
                "The chance a row of `chances` states is not the chance, with the random inputs drawn uniformly, at some combination of the inputs that are not random its cells let in (or does not pass the comparisons it states). The combination comes with it, and the chance there: an exact rational, from counting the points of the random inputs."
            ),
            tr!(
                "表が正しいなら主張を直してください。主張のほうが業務として正しいなら、表の行を直してください。",
                "If the table is right, correct the claim; if the claim is the business truth, correct the table's rows."
            ),
            X_E133,
            &["E070", "W129", "E107"],
        ),
        warn(
            "W129",
            tr!("確率の主張を予算の中で決められませんでした", "A claim of `chances` could not be decided within the budget"),
            tr!(
                "答えが乱数の入力の式で区分の中で変わるところや、乱数の入力とほかの入力を一緒に読むところで、点を一つずつ評価する数が、主張一つの予算（`--budget` の 50 分の 1、既定で 100 万点）を越えるとき。並びが読めないとき、規則が矛盾する点があるときも、決めずにこれを出します。決められない主張は、成り立つとも成り立たないとも言いません。",
                "Where an answer moves inside a box with the random inputs, or the random inputs are read together with the others, the points that have to be run one by one go past the budget of one claim (one 50th of `--budget`'s nodes, a million points by default). A sequence that cannot be read, or a point where the rule contradicts itself, leaves the claim undecided too. A claim that is not decided is said to be neither holding nor broken."
            ),
            tr!(
                "行の乱数でない入力のセルを狭めるか、`--budget` を上げてください。",
                "Narrow the cells of the inputs that are not random, or raise `--budget`."
            ),
            X_W129,
            &["E133", "E109"],
        ),
        warn(
            "W131",
            tr!("確率の関数は、呼ぶたびに規則を何度も評価します", "The chances function runs the rule many times on every call"),
            tr!(
                "乱数の入力の点を掛け合わせた数が 100 万を越え、生成する確率の関数が、呼ぶたびにその数だけ規則を評価するとき。重みで選ぶ規則で乱数の入力を `draw` の行だけが読むときは、関数が重みから数えるので出ません。",
                "The points of the random inputs, multiplied together, are more than a million, and the function generated to count the answers runs the rule that many times on every call. A rule that picks by weight, its random input read by the `draw` line alone, is counted from its weights and does not get it."
            ),
            tr!(
                "頻繁に呼ぶなら、乱数の入力の範囲を狭めてください。重みで選ぶ規則なら、乱数の入力を `draw` の行だけが読むようにしてください。",
                "If it is called often, narrow the ranges of the random inputs; in a rule that picks by weight, let only the `draw` line read the random input."
            ),
            X_W131,
            &["W129", "E108"],
        ),
        err(
            "E067",
            tr!("寄与の形が違います", "A contribution is not shaped as one"),
            tr!(
                "見出しが `contribution` のファイルが、寄与として読めないとき。`to` の行が無い、二行ある、足す先が読めないか check を通らない。`open` の付いていない列挙に `extend` で値を足す、足す先の列挙が別の `enums` のファイルのもの、ある値を二度足す、順序のある列挙に `after` の無い値を足す。`into` の先が開いた値でない、項がどこにも宣言されていないか開いた値より下で決まる。`open` の付いていない出力を決める表や節を足す、開いた出力を決める表や節が、その出力の最後の定義より下の名前を読む。`open` の付いていない前提の塊に行を足す、元の値の行を足す。`after` の輪、二つの寄与が同じ名前、寄与に書けない行（出力、`result`、`apply`、機械、たどる並び、列挙、取り込み、出典、契約、制約、確率の主張）、乱数にした入力。",
                "A file headed `contribution` cannot be read as one. Its `to` line is missing or written twice, or the file it names cannot be read or does not pass check. An `extend` adds a value to an enum not marked `open`, to an enum another `enums` file owns, a value the enum has, or a value with no `after` to an ordered enum. An `into` names something that is not an open value, or a term that is declared nowhere or is decided below the open value. A table or clause decides an output no definition opens, or one of an open output reads a name below the output's last definition. Lines are added to a `prerequisites` block not marked `open`, or a line of one of the block's own values. The `after` lines go round, two contributions share a name, the file holds a line a contribution cannot (outputs, `result`, `apply`, a machine, a sequence it walks, enums, imports, sources, contracts, constraints, claims of chances), or an input is drawn at random."
            ),
            tr!(
                "寄与が足せるのは、足す先が `open` で開いた場所だけです。開いていなければ、足す先の作り手に開いてもらうか、`apply` で読んで自分の規則を書いてください。注に、どこに足せるかが出ます。",
                "A contribution adds only to the places the file it adds to opens with `open`. Where it does not, ask its author to open it, or read it with `apply` and write a rule of your own. The notes say where it can add."
            ),
            X_E067,
            &["E068", "E040", "E033", "E131"],
        )
        .with_files(E067_BASE),
        err(
            "E068",
            tr!("開いた値の形が違います", "An open value is not shaped as one"),
            tr!(
                "`open` の行に合わせ方（`combine sum`・`product`・`max`・`min`）が無いとき、範囲が無いとき、`max` と `min` に `empty` が無いとき、型が合わせ方に合わないとき（足すのは数・お金・量・率、掛けるのは数と率、`max` と `min` はそれに温度と音量）、オプショナルにしたとき、項が値の刻みより細かい刻みのとき。範囲は、寄与を全部合わせても値がその外に出ないという約束で、下の表はこの範囲の全体で検査されます。",
                "An `open` line has no way of combining (`combine sum`, `product`, `max` or `min`), no range, or no `empty` for `max` and `min`; its type is not one the way of combining puts together (a sum takes numbers, money, quantities and rates, a product numbers and rates, `max` and `min` those and temperatures and sound levels); it is optional; or a term is on a finer step than the value. The range is the promise that the value stays inside it whatever is added, and the tables below are checked over the whole of it."
            ),
            tr!(
                "`open <名前> : <型> combine sum|product|max|min [empty <値>] range >=… <=…` の形にしてください。`empty` は、項が無いときの答えで、`max` ならいちばん小さい答え（`min` ならいちばん大きい答え）として合わさります。細かい項は、値の刻みに丸めてから足してください。",
                "Write it `open <name> : <type> combine sum|product|max|min [empty <value>] range >=… <=…`. `empty` is the answer when there is no term, and it takes part as the smallest answer for `max` (the largest for `min`). Round a finer term to the value's step before adding it."
            ),
            X_E068,
            &["E067", "E131"],
        ),
        err(
            "E131",
            tr!("合わせた値が、宣言した範囲を出ます", "A value put together leaves the range it declares"),
            tr!(
                "開いた値の項（規則の項と、寄与が足した項）を合わせ方どおりに合わせた値の区間が、`open` の行の範囲を出るとき。注に、項ごとの区間と、どの寄与の項かと、範囲の外にする入力の例が出ます。範囲は、どの寄与が来ても値がその外に出ないという約束で、下の表はこの範囲の全体で検査されているので、範囲を出た値には、検査されていない答えが返ります。",
                "The interval of an open value's terms — the rule's and those contributions add — put together as its `combine` says, leaves the range on the `open` line. The notes give each term's interval, the contribution each comes from, and an input that takes the value out of the range. The range is the promise that the value stays inside it whatever is added, and the tables below are checked over the whole of it; a value outside it would get an answer nothing checked."
            ),
            tr!(
                "範囲を広げるか（下の表は、広げた範囲で検査し直されます）、寄与が足す項を狭めてください。どちらにするかは業務の判断なので、`fix.text` はありません。",
                "Widen the range (the tables below are then checked over the wider range), or narrow what the contribution adds. Which is a business decision, so there is no `fix.text`."
            ),
            X_E131,
            &["E068", "E112", "E067"],
        )
        .with_files(E131_BASE),
        warn(
            "W132",
            tr!("開いた値の項が、合わせた値を変えません", "A term of an open value changes nothing"),
            tr!(
                "開いた値の項が、どの値を取っても合わせた値を変えられないとき。`max` で項の値が全部 `empty` の値以下のとき、`min` で全部 `empty` の値以上のとき、`sum` でいつも 0 のとき、`product` でいつも 1 のとき。`max` と `min` の `empty` は、項が無いときだけの値ではなく、いつも合わせに加わる出発の値なので、上限を `max` で書くと、項は黙って効かなくなります。寄与が足した項でも、この規則の項でも言います。",
                "A term of an open value cannot move the value whatever it takes: in a `max`, every value of the term is at or below the `empty` value; in a `min`, at or above it; in a `sum`, it is always 0; in a `product`, always 1. The `empty` value of `max` and `min` is the value the terms start from, and it always takes part, not only when there is no term, so a cap written as a `max` leaves its terms with nothing to do, in silence. It is said of a term a contribution adds and of a term of the rule alike."
            ),
            tr!(
                "`max` の項が上限を決めるなら、合わせ方を `min` にしてください（`min` の項が下限を決めるなら `max` に）。`empty` を越える（`min` なら下回る）ことがある項なら、その値を書いてください。要らない項なら外してください。",
                "If a term of a `max` sets a cap, put the value together with `min` (and with `max` if a term of a `min` sets a floor). If the term can pass the `empty` value — above it for a `max`, below it for a `min` — write the values that do. If it is not needed, take it out."
            ),
            X_W132,
            &["E068", "E131"],
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
    let repro = e.repro(crate::i18n::current());
    o.push_str(&indent(repro.example));
    for (name, text) in repro.files {
        o.push_str(&tr!("\n隣に置くファイル `{name}`\n", "\nThe file `{name}` beside it\n"));
        o.push_str(&indent(text));
    }
    for a in &e.also {
        o.push_str(&format!("\n{}\n", a.what));
        let r = a.repro(crate::i18n::current());
        o.push_str(&indent(r.example));
        for (name, text) in r.files {
            o.push_str(&tr!("\n隣に置くファイル `{name}`\n", "\nThe file `{name}` beside it\n"));
            o.push_str(&indent(text));
        }
    }
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
    let repro = e.repro(crate::i18n::current());
    o.push_str(repro.example);
    o.push_str("```\n");
    for (name, text) in repro.files {
        o.push_str(&tr!("\n隣に置く `{name}`:\n\n", "\nWith `{name}` beside it:\n\n"));
        o.push_str("```proto\n");
        o.push_str(text);
        o.push_str("```\n");
    }
    for a in &e.also {
        o.push_str(&format!("\n**{}**:\n\n```rule\n", a.what));
        let r = a.repro(crate::i18n::current());
        o.push_str(r.example);
        o.push_str("```\n");
        for (name, text) in r.files {
            o.push_str(&tr!("\n隣に置く `{name}`:\n\n", "\nWith `{name}` beside it:\n\n"));
            o.push_str(if name.ends_with(".proto") { "```proto\n" } else { "```rule\n" });
            o.push_str(text);
            o.push_str("```\n");
        }
    }
    if !e.related.is_empty() {
        let links: Vec<String> =
            e.related.iter().map(|c| format!("[{c}](#{})", c.to_ascii_lowercase())).collect();
        o.push_str(&tr!("\n関係するコード: {}\n", "\nRelated codes: {}\n", links.join(", ")));
    }
    o
}

/// `rulec explain <CODE> --format json`. Language independent in its keys; the prose
/// fields follow `--lang`, and so does the reproduction (`example` and `files`) of an entry
/// that has an English twin.
pub fn render_json(e: &Entry) -> String {
    let repro = e.repro(crate::i18n::current());
    json::Obj::new()
        .str("code", e.code)
        .str("severity", severity_word(e.severity))
        .str("title", &e.title)
        .str("when", &e.when)
        .str("fix", &e.fix)
        .str("example", repro.example)
        .opt_raw("budget", e.budget.map(|b| b.to_string()))
        .raw(
            "files",
            format!(
                "[{}]",
                repro
                    .files
                    .iter()
                    .map(|(n, t)| json::Obj::new().str("name", n).str("text", t).finish())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        )
        // Another form with its reproduction, where the entry has one (E102, §15.189).
        .opt_raw(
            "also",
            (!e.also.is_empty()).then(|| {
                let lang = crate::i18n::current();
                let each: Vec<String> = e.also.iter().map(|a| json::Obj::new().str("what", &a.what).str("example", a.repro(lang).example).finish()).collect();
                format!("[{}]", each.join(","))
            }),
        )
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
        "rulec が出しうるコードの全部について、いつ出るかと、どう直すかを書いています。コードと JSON の形は安定 API で、変わるのは文面の改善だけです。一件だけ読むときは `rulec explain E101` を使ってください。\n\n",
        "Every code rulec can print, what makes it appear, and how to fix it. The code and the JSON shape are a stable API; only the prose improves. For one of them: `rulec explain E101`.\n\n"
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

// ── The reproductions of the codes about chances (§15.224–§15.226) ─────────────

/// E070: a number drawn with no range to draw it from.
const X_E070: &str = r##"rule t(t) v1

inputs
  roll(roll) : number  random

outputs
  r(r) : bool

table j(j)
policy unique
| roll | -> r(r) : bool |
| <5   | true           |
| >=5  | false          |
"##;
/// E073: a pick by weight that does not say which value it draws.
const X_E073: &str = r##"rule t(t) v1

enum v(v) = skip(skip) | pick(pick)

elements os(os)
  ix(ix) : number  range >=1 <=9
  ok(ok) : bool
  w(w)   : number  range >=0 <=10

inputs
  roll(roll) : number  range >=0 <=99  random

outputs
  p(p) : number  round down(1)

table j(j)
policy unique
| ok    | -> v(v) : v |
| true  | pick        |
| false | skip        |

fold v over os
  skip      -> next
  pick      -> take_weighted ix by w
  empty     -> 0
  exhausted -> held
"##;
/// E133: three values of ten answer true, and the claim says one in three.
const X_E133: &str = r##"rule t(t) v1

inputs
  roll(roll) : number  range >=0 <=9  random

outputs
  r(r) : bool

table j(j)
policy unique
| roll | -> r(r) : bool |
| <3   | true           |
| >=3  | false          |

chances
| -> r | chance |
| true | 1/3    |
"##;
/// W129: the drawn value and another input read together, over more points than one claim's budget.
const X_W129: &str = r##"rule t(t) v1

inputs
  roll(roll) : number  range >=0 <=999  random
  a(a)       : number  range >=0 <=9999

outputs
  r(r) : bool

derive d(d) : number = roll - a  range >=-9999 <=999

table j(j)
policy unique
| d   | -> r(r) : bool |
| <0  | true           |
| >=0 | false          |

chances
| a | -> r | chance |
| - | true | >=0    |
"##;
/// W131: ten million points for the function that counts the answers to run on every call.
const X_W131: &str = r##"rule t(t) v1

inputs
  a(a) : number  range >=0 <=999  random
  b(b) : number  range >=0 <=9999  random

outputs
  r(r) : bool

table j(j)
policy unique
| a     | b | -> r(r) : bool |
| <500  | - | true           |
| >=500 | - | false          |
"##;
/// E108, the points of the random inputs: 10^20 of them, past the 2^62 a count can hold.
const X_E108_RANDOM: &str = r##"rule t(t) v1

inputs
  a(a) : number  range >=0 <=9999999999  random
  b(b) : number  range >=0 <=9999999999  random

outputs
  r(r) : bool

table j(j)
policy unique
| a   | b | -> r(r) : bool |
| <5  | - | true           |
| >=5 | - | false          |
"##;
