// @HEAD@
//! What every module of kaname in this package shares (kaname's DESIGN 10.1, 36.7): exact fractions,
//! the arithmetic held to int64, rulec's five ways of rounding, dates, how a value is written in a
//! message, the helpers of the dicts, the sets, the lists and the strings, and the error the modules
//! return. It uses the standard library alone.
//!
//! A function of a module that can fail returns `Result<_, KanameError>`, and the places inside it
//! that fail return the error with `?`. A function that cannot fail returns its value, and panics
//! where a host gives it an argument outside what its parameters declare; its `check_<name>` says so
//! first, as an error.

#![allow(dead_code)]

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The language the messages are written in: the --lang of ritsu gen.
const LANG: &str = "@LANG@";

// ── The error ───────────────────────────────────────────────────────────────

/// The kind of a `KanameError` (kaname's DESIGN 4.2): `input` (an argument a host gave is outside
/// what the function takes), or a kind of the effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Input,
    Rule,
    Date,
    Range,
    Overflow,
    Zero,
}

impl Kind {
    /// The kind as kaname names it.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Input => "input",
            Kind::Rule => "rule",
            Kind::Date => "date",
            Kind::Range => "range",
            Kind::Overflow => "overflow",
            Kind::Zero => "zero",
        }
    }

    /// The kind of its name; None for a name of no kind.
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "input" => Kind::Input,
            "rule" => Kind::Rule,
            "date" => Kind::Date,
            "range" => Kind::Range,
            "overflow" => Kind::Overflow,
            "zero" => Kind::Zero,
            _ => return None,
        })
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a function of kaname stops: its kind, where in the `.kn` it stopped (the module and the
/// line), the message, and what the code of a rule or of a date returned, when it is that code's
/// refusal.
#[derive(Debug)]
pub struct KanameError {
    pub kind: Kind,
    pub module: &'static str,
    pub line: u32,
    pub message: String,
    pub cause: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl fmt::Display for KanameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for KanameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.cause {
            Some(e) => Some(e.as_ref()),
            None => None,
        }
    }
}

/// The error of `kind` at `line` of `module`.
pub fn fail(kind: &str, module: &'static str, line: u32, message: impl AsRef<str>) -> KanameError {
    KanameError { kind: Kind::parse(kind).unwrap_or(Kind::Input), module, line, message: spaced(message.as_ref()), cause: None }
}

/// The error of `kind` at `line` of `module`, returned: a place that fails whatever it is given.
pub fn fails(kind: &str, module: &'static str, line: u32, message: impl AsRef<str>) -> Result<(), KanameError> {
    Err(fail(kind, module, line, message))
}

/// The error of `kind` at `line` of `module`, caused by `cause`: what a rule's or a date's generated
/// code returned.
pub fn refused(kind: &str, module: &'static str, line: u32, message: impl AsRef<str>, cause: impl std::error::Error + Send + Sync + 'static) -> KanameError {
    KanameError { cause: Some(Box::new(cause)), ..fail(kind, module, line, message) }
}

/// Where a value that leaves int64 or a divisor of 0 is said: the module, the line, the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct At {
    pub module: &'static str,
    pub line: u32,
    pub message: &'static str,
}

impl At {
    fn fail(self, kind: &str) -> KanameError {
        fail(kind, self.module, self.line, self.message)
    }
}

/// A message as kaname writes it: an English sentence begins with a capital; Japanese has a space
/// between an ASCII word and the Japanese beside it, outside backquotes.
pub fn spaced(s: &str) -> String {
    if LANG != "ja" {
        let mut cs = s.chars();
        return match cs.next() {
            Some(c) if c.is_ascii_lowercase() => format!("{}{}", c.to_ascii_uppercase(), cs.as_str()),
            _ => s.to_string(),
        };
    }
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut code = false;
    for (i, &c) in cs.iter().enumerate() {
        if c == '`' {
            code = !code;
        }
        if i > 0 && !code {
            let p = cs[i - 1];
            if (latin(p) && japanese(c)) || (japanese(p) && latin(c)) {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out
}

fn latin(c: char) -> bool {
    c.is_ascii_alphabetic()
}

fn japanese(c: char) -> bool {
    let n = c as u32;
    (0x3040..=0x30ff).contains(&n) || (0x3400..=0x4dbf).contains(&n) || (0x4e00..=0x9fff).contains(&n) || n == 0x3005
}

// ── Whole numbers and fractions ──────────────────────────────────────────────

/// A value that is not a whole number of its unit: an exact fraction in lowest terms, its
/// denominator above 0. Every fraction the modules hold keeps the rule of int64 (kaname's DESIGN
/// 5.6), so the products of two of them, which the operations work in, fit in `i128`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Q {
    num: i128,
    den: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a as i128
}

/// A value of the work in `i128`, which no fraction that keeps the rule of int64 leaves.
fn work(x: Option<i128>) -> i128 {
    x.expect("kaname: a fraction past the rule of int64")
}

impl Q {
    /// n/d in lowest terms; d is not 0.
    fn new(n: i128, d: i128) -> Q {
        assert!(d != 0, "kaname: a fraction over 0");
        let g = gcd(n, d).max(1);
        let (n, d) = (n / g, d / g);
        if d < 0 { Q { num: -n, den: -d } } else { Q { num: n, den: d } }
    }

    /// The numerator, in lowest terms.
    pub fn num(self) -> i128 {
        self.num
    }

    /// The denominator, above 0.
    pub fn den(self) -> i128 {
        self.den
    }

    fn is_int(self) -> bool {
        self.den == 1
    }

    /// Whether the fraction keeps the rule of int64: its numerator in int64, its denominator from 1
    /// to 2^63 - 1.
    fn fits(self) -> bool {
        (i64::MIN as i128..=i64::MAX as i128).contains(&self.num) && self.den <= i64::MAX as i128
    }
}

impl Ord for Q {
    fn cmp(&self, o: &Q) -> Ordering {
        work(self.num.checked_mul(o.den)).cmp(&work(o.num.checked_mul(self.den)))
    }
}

impl PartialOrd for Q {
    fn partial_cmp(&self, o: &Q) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl fmt::Display for Q {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&rat_text(*self))
    }
}

/// n/d.
pub fn frac(n: i128, d: i128) -> Q {
    Q::new(n, d)
}

/// A whole number as a fraction.
pub fn of(x: i64) -> Q {
    Q { num: x as i128, den: 1 }
}

pub fn q_add(a: Q, b: Q) -> Q {
    let n = a.num.checked_mul(b.den).and_then(|x| b.num.checked_mul(a.den).and_then(|y| x.checked_add(y)));
    Q::new(work(n), work(a.den.checked_mul(b.den)))
}

pub fn q_sub(a: Q, b: Q) -> Q {
    let n = a.num.checked_mul(b.den).and_then(|x| b.num.checked_mul(a.den).and_then(|y| x.checked_sub(y)));
    Q::new(work(n), work(a.den.checked_mul(b.den)))
}

pub fn q_mul(a: Q, b: Q) -> Q {
    Q::new(work(a.num.checked_mul(b.num)), work(a.den.checked_mul(b.den)))
}

pub fn q_neg(a: Q) -> Q {
    Q { num: -a.num, den: a.den }
}

/// a / b, b not 0: where it can be, the module asks [`over`] first.
pub fn q_div(a: Q, b: Q) -> Q {
    assert!(b.num != 0, "kaname: a division by 0");
    Q::new(work(a.num.checked_mul(b.den)), work(a.den.checked_mul(b.num)))
}

/// A fraction whose type says it is a whole number of its unit, as that number.
pub fn whole(a: Q) -> i64 {
    assert!(a.is_int() && a.fits(), "kaname: {} is no whole number of int64", rat_text(a));
    a.num as i64
}

/// A rate, as the whole number of its step it travels as.
pub fn steps(a: Q, step: Q) -> i64 {
    whole(q_div(a, step))
}

/// The rate that travels as `k` steps of `step`.
pub fn rate(k: i64, step: Q) -> Q {
    q_mul(of(k), step)
}

// ── The rule of int64 (kaname's DESIGN 5.6) ──────────────────────────────────

/// a + b, held to int64.
pub fn add(a: i64, b: i64, module: &'static str, line: u32, message: &str) -> Result<i64, KanameError> {
    a.checked_add(b).ok_or_else(|| fail("overflow", module, line, message))
}

/// a - b, held to int64.
pub fn sub(a: i64, b: i64, module: &'static str, line: u32, message: &str) -> Result<i64, KanameError> {
    a.checked_sub(b).ok_or_else(|| fail("overflow", module, line, message))
}

/// a × b, held to int64.
pub fn mul(a: i64, b: i64, module: &'static str, line: u32, message: &str) -> Result<i64, KanameError> {
    a.checked_mul(b).ok_or_else(|| fail("overflow", module, line, message))
}

/// -a, held to int64.
pub fn neg(a: i64, module: &'static str, line: u32, message: &str) -> Result<i64, KanameError> {
    a.checked_neg().ok_or_else(|| fail("overflow", module, line, message))
}

/// A fraction held to int64: its numerator in int64, and its denominator from 1 to 2^63 - 1.
pub fn held(x: Q, module: &'static str, line: u32, message: &str) -> Result<Q, KanameError> {
    if x.fits() { Ok(x) } else { Err(fail("overflow", module, line, message)) }
}

/// a / b, with the error zero when b is 0.
pub fn over(a: Q, b: Q, module: &'static str, line: u32, message: &str) -> Result<Q, KanameError> {
    if b.num == 0 {
        return Err(fail("zero", module, line, message));
    }
    Ok(q_div(a, b))
}

// ── Rounding (kaname's DESIGN 3.3) ──────────────────────────────────────────

/// x rounded `way` (up, down, half_up, half_down or half_even) to a whole number of the step g (above
/// 0): the steps are |n·gd| over d·gn, the quotient taken or one past it as `way` says, the sign put
/// back. Rust's own division, which goes toward 0, decides no fraction's way.
pub fn round(x: Q, way: &str, g: Q) -> Q {
    let top = work(x.num.checked_mul(g.den));
    let y = work(x.den.checked_mul(g.num)).unsigned_abs();
    let a = top.unsigned_abs();
    let (mut k, r) = (a / y, a % y);
    if r != 0 {
        let twice = r * 2;
        let past = match way {
            "up" => true,
            "half_up" => twice >= y,
            "half_down" => twice > y,
            "half_even" => twice > y || (twice == y && k % 2 == 1),
            _ => false,
        };
        if past {
            k += 1;
        }
    }
    let k = k as i128;
    q_mul(Q { num: if top < 0 { -k } else { k }, den: 1 }, g)
}

/// allocate(t, c, s): t × c / s rounded toward 0 to a whole unit (rulec's §15.102), where the analysis
/// shows no value on the way leaves int64 and s is not 0.
pub fn allocate(t: Q, c: Q, s: Q) -> Q {
    round(q_div(q_mul(t, c), s), "down", of(1))
}

/// allocate(t, c, s), each value on the way held to int64 where `overflow` says, and the error zero
/// where `zero` says.
pub fn allocate_at(t: Q, c: Q, s: Q, overflow: Option<At>, zero: Option<At>) -> Result<Q, KanameError> {
    let held = |x: Q| -> Result<Q, KanameError> {
        match overflow {
            Some(at) if !x.fits() => Err(at.fail("overflow")),
            _ => Ok(x),
        }
    };
    let p = held(q_mul(t, c))?;
    if let (Some(at), 0) = (zero, s.num) {
        return Err(at.fail("zero"));
    }
    let part = held(q_div(p, s))?;
    held(round(part, "down", of(1)))
}

/// The whole part of x, toward −∞.
fn floor(x: Q) -> i128 {
    x.num.div_euclid(x.den)
}

/// mod(x, n): the whole part of x (toward −∞) modulo the positive whole n, from 0 to n − 1.
pub fn r#mod(x: Q, n: i64) -> i64 {
    floor(x).rem_euclid(n as i128) as i64
}

/// mod(x, n) of a divisor that is a value the analysis shows is never 0: the whole part of x (toward
/// −∞) modulo n, from 0 to |n| − 1 (Euclid's, kaname's DESIGN 24.3).
pub fn mod_of(x: Q, n: i64) -> i64 {
    assert!(n != 0, "kaname: mod of 0");
    floor(x).rem_euclid((n as i128).abs()) as i64
}

/// [`mod_of`], with the error zero where n is 0.
pub fn mod_of_at(x: Q, n: i64, module: &'static str, line: u32, message: &str) -> Result<i64, KanameError> {
    if n == 0 {
        return Err(fail("zero", module, line, message));
    }
    Ok(mod_of(x, n))
}

/// isqrt(x): the largest whole number whose square is not above x; None below 0.
pub fn isqrt(x: Q) -> Option<i64> {
    if x.num < 0 {
        return None;
    }
    let v = floor(x) as u128;
    let mut r = (v as f64).sqrt() as u128;
    while r * r > v {
        r -= 1;
    }
    while (r + 1) * (r + 1) <= v {
        r += 1;
    }
    Some(r as i64)
}

/// isqrt(x) where the analysis shows x is 0 or more.
pub fn root(x: Q) -> i64 {
    isqrt(x).unwrap_or_else(|| panic!("kaname: isqrt of {}", rat_text(x)))
}

// ── Dates (kaname's DESIGN 3.4) ──────────────────────────────────────────────

/// A day of the calendar, with no time zone. A host may give a day of no calendar (2026-02-30,
/// 2026-13-01), which the entry finds outside the range of the parameter: the days 0001-01-01 to
/// 9999-12-31 where the range leaves an end to the type (kaname's DESIGN 22.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// The day number of the date: days since 1970-01-01 (Howard Hinnant's days_from_civil).
    pub fn days(self) -> i64 {
        let (mut y, m, d) = (self.year as i64, self.month as i64, self.day as i64);
        if m <= 2 {
            y -= 1;
        }
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    /// Whether the date is a day of the calendar from 0001-01-01 to 9999-12-31: what a date whose
    /// range is not written takes (kaname's DESIGN 22.5).
    pub fn valid(self) -> bool {
        (1..=9999).contains(&self.year) && self.on_calendar()
    }

    /// Whether the date is a day of the calendar, of any year: 2026-02-30 and 2026-13-01 are none.
    pub fn on_calendar(self) -> bool {
        (1..=12).contains(&self.month) && self.day >= 1 && date_of(self.days()) == self
    }
}

/// The date of a day number (civil_from_days).
pub fn date_of(z: i64) -> Date {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    Date { year: (if m <= 2 { y + 1 } else { y }) as i32, month: m as u32, day: d as u32 }
}

impl fmt::Display for Date {
    /// YYYY-MM-DD.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// b - a: the days from the date a to the date b.
pub fn between(b: Date, a: Date) -> i64 {
    b.days() - a.days()
}

// ── Values as a message writes them ──────────────────────────────────────────

/// A fraction as a message writes it: a whole number as it is, a decimal where the denominator is
/// made of 2s and 5s, and n/d otherwise.
pub fn rat_text(x: Q) -> String {
    if x.is_int() {
        return x.num.to_string();
    }
    let sign = if x.num < 0 { "-" } else { "" };
    let num = x.num.unsigned_abs();
    let den = x.den as u128;
    let mut rest = den;
    let (mut twos, mut fives) = (0u32, 0u32);
    while rest.is_multiple_of(2) {
        rest /= 2;
        twos += 1;
    }
    while rest.is_multiple_of(5) {
        rest /= 5;
        fives += 1;
    }
    let places = twos.max(fives);
    // as ritsu writes it: a decimal past 128 bits is written as the fraction
    let scale = if rest == 1 && places <= 38 { 10u128.checked_pow(places) } else { None };
    if let Some((scale, scaled)) = scale.and_then(|k| num.checked_mul(k).map(|x| (k, x))) {
        let v = scaled / den;
        let (w, f) = (v / scale, v % scale);
        return format!("{sign}{w}.{f:0width$}", width = places as usize);
    }
    format!("{sign}{num}/{den}")
}

/// A number as a literal of its unit: 1150GBPc, 0.5%, 30.
pub fn num_text(x: Q, unit: &str) -> String {
    if unit == "%" {
        return format!("{}%", rat_text(q_mul(x, of(100))));
    }
    format!("{}{unit}", rat_text(x))
}

/// A whole number with its unit.
pub fn int_text(x: i64, unit: &str) -> String {
    num_text(of(x), unit)
}

/// true or false.
pub fn bool_text(b: bool) -> String {
    b.to_string()
}

/// Where a value a function is given or answers is outside what its place declares: the place (a
/// parameter, and the field or the element of it), the value as a message writes it, and the range;
/// or, for a list, a dict, a set or a string, its count (of a string's code points where `chars`,
/// kaname's DESIGN 25.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outside {
    pub path: String,
    pub shown: String,
    pub range: String,
    pub count: Option<i64>,
    pub chars: bool,
}

/// A value outside its range.
pub fn outside(path: impl Into<String>, shown: String, range: &str) -> Outside {
    Outside { path: path.into(), shown, range: range.to_string(), count: None, chars: false }
}

/// A list, a dict or a set of `n` elements, outside its count.
pub fn outside_count(path: impl Into<String>, n: usize, range: &str) -> Outside {
    Outside { path: path.into(), shown: String::new(), range: range.to_string(), count: Some(n as i64), chars: false }
}

/// A string of `n` code points, outside its count.
pub fn outside_chars(path: impl Into<String>, n: i64, range: &str) -> Outside {
    Outside { path: path.into(), shown: String::new(), range: range.to_string(), count: Some(n), chars: true }
}

/// A count as a message writes it.
pub fn itoa(n: Option<i64>) -> String {
    n.map(|n| n.to_string()).unwrap_or_default()
}

/// The path of the i-th element of `what`: what[i].
pub fn elem(what: &str, i: usize) -> String {
    format!("{what}[{i}]")
}

// ── Records the built-in functions answer (kaname's DESIGN 3.6, 24.3) ───────

/// An element of what running_totals answers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Running<T, W> {
    pub item: T,
    pub before: W,
    pub upto: W,
    pub total: W,
}

/// An entry of a dict: an element of what entries answers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry<K, V> {
    pub key: K,
    pub value: V,
}

/// An element of what indexed answers: the index from 0, and the element.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Indexed<T> {
    pub index: i64,
    pub item: T,
}

// ── Dicts and sets (kaname's DESIGN 24) ──────────────────────────────────────

/// What holds keys in their order: a dict's keys, a set's members.
pub trait Keyed<K> {
    /// The keys, in their order.
    fn keys_in_order(&self) -> Vec<K>;
}

impl<K: Clone, V> Keyed<K> for BTreeMap<K, V> {
    fn keys_in_order(&self) -> Vec<K> {
        self.keys().cloned().collect()
    }
}

impl<K: Clone> Keyed<K> for BTreeSet<K> {
    fn keys_in_order(&self) -> Vec<K> {
        self.iter().cloned().collect()
    }
}

/// The keys of a dict, or the members of a set, in their order.
pub fn keys_of<K, M: Keyed<K>>(m: &M) -> Vec<K> {
    m.keys_in_order()
}

/// The values of a dict, in the order of their keys.
pub fn values_of<K, V: Clone>(m: &BTreeMap<K, V>) -> Vec<V> {
    m.values().cloned().collect()
}

/// The pairs of a dict as entries, in the order of their keys.
pub fn entries_of<K: Clone, V: Clone>(m: &BTreeMap<K, V>) -> Vec<Entry<K, V>> {
    m.iter().map(|(k, v)| Entry { key: k.clone(), value: v.clone() }).collect()
}

/// A dict with the key put in, or its value replaced.
pub fn put<K: Ord + Clone, V: Clone>(m: &BTreeMap<K, V>, k: K, v: V) -> BTreeMap<K, V> {
    let mut out = m.clone();
    out.insert(k, v);
    out
}

/// A dict with the key taken out.
pub fn remove<K: Ord + Clone + Borrow<B>, B: Ord + ?Sized, V: Clone>(m: &BTreeMap<K, V>, k: &B) -> BTreeMap<K, V> {
    let mut out = m.clone();
    out.remove(k);
    out
}

/// The pairs of both, b's value for a key of both.
pub fn merge<K: Ord + Clone, V: Clone>(a: &BTreeMap<K, V>, b: &BTreeMap<K, V>) -> BTreeMap<K, V> {
    let mut out = a.clone();
    for (k, v) in b {
        out.insert(k.clone(), v.clone());
    }
    out
}

/// A set of these members.
pub fn set_of<T: Ord + Clone>(xs: &[T]) -> BTreeSet<T> {
    xs.iter().cloned().collect()
}

/// A set with the member put in.
pub fn insert<T: Ord + Clone>(s: &BTreeSet<T>, x: T) -> BTreeSet<T> {
    let mut out = s.clone();
    out.insert(x);
    out
}

/// A set with the member taken out.
pub fn without<T: Ord + Clone + Borrow<B>, B: Ord + ?Sized>(s: &BTreeSet<T>, x: &B) -> BTreeSet<T> {
    let mut out = s.clone();
    out.remove(x);
    out
}

/// The members of either.
pub fn union<T: Ord + Clone>(a: &BTreeSet<T>, b: &BTreeSet<T>) -> BTreeSet<T> {
    a.union(b).cloned().collect()
}

/// The members of both.
pub fn intersect<T: Ord + Clone>(a: &BTreeSet<T>, b: &BTreeSet<T>) -> BTreeSet<T> {
    a.intersection(b).cloned().collect()
}

/// The members of a that are not of b.
pub fn difference<T: Ord + Clone>(a: &BTreeSet<T>, b: &BTreeSet<T>) -> BTreeSet<T> {
    a.difference(b).cloned().collect()
}

// ── Lists (kaname's DESIGN 24.3) ─────────────────────────────────────────────

/// A whole number cut to 0 and to n: where a list is cut.
fn cut(i: i64, n: usize) -> usize {
    if i < 0 { 0 } else { (i as u64).min(n as u64) as usize }
}

/// The element at i (from 0); None past either end.
pub fn elem_at<T: Clone>(xs: &[T], i: i64) -> Option<T> {
    if i < 0 { None } else { xs.get(i as usize).cloned() }
}

/// The elements from a up to before b, both cut to the list.
pub fn slice<T: Clone>(xs: &[T], a: i64, b: i64) -> Vec<T> {
    let (i, j) = (cut(a, xs.len()), cut(b, xs.len()));
    if i >= j { Vec::new() } else { xs[i..j].to_vec() }
}

/// The first n elements.
pub fn take<T: Clone>(xs: &[T], n: i64) -> Vec<T> {
    slice(xs, 0, n)
}

/// The elements past the first n.
pub fn drop<T: Clone>(xs: &[T], n: i64) -> Vec<T> {
    xs[cut(n, xs.len())..].to_vec()
}

/// The elements from the last.
pub fn reverse<T: Clone>(xs: &[T]) -> Vec<T> {
    xs.iter().rev().cloned().collect()
}

/// The list with x after its elements.
pub fn append<T: Clone>(xs: &[T], x: T) -> Vec<T> {
    let mut out = Vec::with_capacity(xs.len() + 1);
    out.extend_from_slice(xs);
    out.push(x);
    out
}

/// The lists of a list one after another.
pub fn flatten<T: Clone>(xss: &[Vec<T>]) -> Vec<T> {
    xss.concat()
}

/// The elements in their order, two of one value in the order of the list.
pub fn sorted<T: Ord + Clone>(xs: &[T]) -> Vec<T> {
    let mut out = xs.to_vec();
    out.sort();
    out
}

/// The elements in the order of their keys, from the greatest where `desc`, two of one key in the
/// order of the list either way (`sort_by` is stable).
pub fn by_key<K: Ord, T>(mut ps: Vec<(K, T)>, desc: bool) -> Vec<T> {
    if desc {
        ps.sort_by(|a, b| b.0.cmp(&a.0));
    } else {
        ps.sort_by(|a, b| a.0.cmp(&b.0));
    }
    ps.into_iter().map(|(_, x)| x).collect()
}

/// The first of each value, in the order of the list.
pub fn distinct<T: Ord + Clone>(xs: &[T]) -> Vec<T> {
    let mut seen = BTreeSet::new();
    xs.iter().filter(|x| seen.insert((*x).clone())).cloned().collect()
}

/// numbers(a, b): a to b, both in; none where b is before a, where the analysis shows the count keeps
/// the rule of int64.
pub fn numbers(a: i64, b: i64) -> Vec<i64> {
    if b < a { Vec::new() } else { (a..=b).collect() }
}

/// [`numbers`], the count held to int64.
pub fn numbers_at(a: i64, b: i64, module: &'static str, line: u32, message: &str) -> Result<Vec<i64>, KanameError> {
    if b >= a && (b as i128 - a as i128) > i64::MAX as i128 - 1 {
        return Err(fail("overflow", module, line, message));
    }
    Ok(numbers(a, b))
}

// ── Strings (kaname's DESIGN 25) ─────────────────────────────────────────────

// A string is a sequence of code points: Rust counts the bytes of UTF-8, so the length, the cuts and
// the places are counted here in chars. The bytes of UTF-8 are in the order of their code points, so
// `<` compares two strings as kaname does, and a search finds what it looks for at the start of a
// code point.

/// Whether a code point is one of Unicode's White_Space, which trim and words take for white space.
fn is_space(c: char) -> bool {
    matches!(c as u32, 0x9..=0xd | 0x20 | 0x85 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000)
}

/// The number of code points.
pub fn cp_count(s: &str) -> i64 {
    s.chars().count() as i64
}

/// The code points from a up to before b, both cut to the string.
pub fn text_slice(s: &str, a: i64, b: i64) -> String {
    let n = s.chars().count();
    let (i, j) = (cut(a, n), cut(b, n));
    if i >= j { String::new() } else { s.chars().skip(i).take(j - i).collect() }
}

/// The first n code points.
pub fn text_take(s: &str, n: i64) -> String {
    text_slice(s, 0, n)
}

/// The code points after the first n.
pub fn text_drop(s: &str, n: i64) -> String {
    s.chars().skip(cut(n, s.chars().count())).collect()
}

/// Where t first stands in s, in code points; 0 for an empty t, None where it is not there.
pub fn index_of(s: &str, t: &str) -> Option<i64> {
    s.find(t).map(|i| s[..i].chars().count() as i64)
}

/// The strings of one code point each.
pub fn chars(s: &str) -> Vec<String> {
    s.chars().map(String::from).collect()
}

/// The code points, as numbers.
pub fn code_points(s: &str) -> Vec<i64> {
    s.chars().map(|c| c as i64).collect()
}

/// The string of these code points; None where one is no Unicode scalar value.
pub fn to_text(ns: &[i64]) -> Option<String> {
    ns.iter().map(|&n| u32::try_from(n).ok().and_then(char::from_u32)).collect()
}

/// s cut at each sep, the empty strings at the ends kept; each code point where sep is empty.
pub fn split(s: &str, sep: &str) -> Vec<String> {
    if sep.is_empty() {
        return chars(s);
    }
    s.split(sep).map(String::from).collect()
}

/// The runs of what is not white space.
pub fn words(s: &str) -> Vec<String> {
    s.split(is_space).filter(|w| !w.is_empty()).map(String::from).collect()
}

/// Every `old` from the left made `new`; s as it is where `old` is empty.
pub fn replace(s: &str, old: &str, new: &str) -> String {
    if old.is_empty() { s.to_string() } else { s.replace(old, new) }
}

/// s without the white space at its ends.
pub fn trim(s: &str) -> String {
    s.trim_matches(is_space).to_string()
}

/// s without the white space at its start.
pub fn trim_start(s: &str) -> String {
    s.trim_start_matches(is_space).to_string()
}

/// s without the white space at its end.
pub fn trim_end(s: &str) -> String {
    s.trim_end_matches(is_space).to_string()
}

/// s with A to Z made a to z, nothing else changed.
pub fn lower_ascii(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// s with a to z made A to Z, nothing else changed.
pub fn upper_ascii(s: &str) -> String {
    s.to_ascii_uppercase()
}

/// `fill` repeated before s (after it, with `at_end`) to make it `width` code points long, the last
/// repetition cut; s as it is where `fill` is empty or s is that long already.
pub fn pad(s: &str, width: i64, fill: &str, at_end: bool) -> String {
    let n = cp_count(s);
    if fill.is_empty() || n >= width {
        return s.to_string();
    }
    let added: String = fill.chars().cycle().take((width - n) as usize).collect();
    if at_end { format!("{s}{added}") } else { format!("{added}{s}") }
}

/// Whether s is one ASCII digit or more, and nothing else.
fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// The number s writes: one + or - at most, then ASCII digits, nothing else, within int64; None
/// otherwise.
pub fn parse_number(s: &str) -> Option<i64> {
    let d = s.strip_prefix(['+', '-']).unwrap_or(s);
    if !digits(d) {
        return None;
    }
    s.parse::<i64>().ok()
}

/// The date s writes: YYYY-MM-DD alone, a day of 0001-01-01 to 9999-12-31; None otherwise.
pub fn parse_text(s: &str) -> Option<Date> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !digits(&s[0..4]) || !digits(&s[5..7]) || !digits(&s[8..10]) {
        return None;
    }
    let d = Date { year: s[0..4].parse().ok()?, month: s[5..7].parse().ok()?, day: s[8..10].parse().ok()? };
    d.valid().then_some(d)
}

/// Whether s begins with p.
pub fn starts_with(s: &str, p: &str) -> bool {
    s.starts_with(p)
}

/// Whether s ends with t.
pub fn ends_with(s: &str, t: &str) -> bool {
    s.ends_with(t)
}

/// Whether t stands somewhere in s.
pub fn text_contains(s: &str, t: &str) -> bool {
    s.contains(t)
}
