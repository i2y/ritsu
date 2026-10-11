//! The helpers of the three operations §15.213–§15.215 added to the expressions: a rounding of a
//! quotient by a value, `mod` and `isqrt`. Each target gets the same algorithm written in its own
//! language, as the five rounding modes are (§7.3), and only a rule that uses one gets it: the
//! code generated from every other rule is what it was.
//!
//! The quotient helpers take the two integers the expression brings to one step and answer the
//! number of grid steps: the divisor is never zero (E132), a negative divisor turns both signs
//! first, and the direction is then about the magnitude, as in the five modes. `mod` takes a
//! positive whole constant and answers 0 up to it less one, whatever the sign. `isqrt` searches
//! for the largest whole number whose square does not pass the value, by halves, so that no
//! square it tries passes int64; the value is never negative (E139). No language's own square
//! root is used: Python's `math.isqrt` would bring `math` into a module that a compiler of typed
//! Python takes ahead of time, and a floating root is not exact past 2^53.
//!
//! The unit vectors (§8.5) go into each language's `_round_test` file beside the five modes',
//! for the helpers the rule uses: negative dividends and divisors, exact halves, exact quotients,
//! and values at the top of int64.

use super::mode_fn;
use crate::ast::{Arm, Expr, Item, RuleFile};
use crate::num::{Rat, RoundMode, RoundTo};

/// Which of the helpers a rule's expressions call.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Uses {
    /// The rounding modes a quotient by a value is rounded with, in the order of §7.3's list.
    pub div: Vec<RoundMode>,
    pub modulo: bool,
    pub root: bool,
}

const MODES: [RoundMode; 5] = [RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::HalfDown, RoundMode::Bankers];

impl Uses {
    /// The helpers the expressions of `f` call: its derived values, definitions and result, and the
    /// expressions of its walk.
    pub fn of(f: &RuleFile) -> Uses {
        let mut exprs: Vec<&Expr> = Vec::new();
        for it in &f.items {
            match it {
                Item::Derived(d) => exprs.push(&d.expr),
                Item::Define(d) => exprs.push(&d.expr),
                _ => {}
            }
        }
        if let Some(r) = &f.result {
            exprs.push(&r.expr);
        }
        if let Some(fd) = &f.fold {
            exprs.extend(fd.empty.iter());
            exprs.extend(fd.exhausted.iter());
            for (_, arm, _) in &fd.arms {
                match arm {
                    Arm::Stop(Some(e)) => exprs.push(e),
                    Arm::Take { expr, .. } => exprs.push(expr),
                    Arm::KeepMax { expr, key } => {
                        exprs.push(expr);
                        exprs.push(key);
                    }
                    _ => {}
                }
            }
        }
        let mut found: Vec<RoundMode> = Vec::new();
        let mut u = Uses::default();
        fn walk(e: &Expr, found: &mut Vec<RoundMode>, u: &mut Uses) {
            match e {
                Expr::Call(name, args, _) => {
                    if crate::types::var_division(name, args).is_some() {
                        if let Some(m) = RoundMode::parse(name) {
                            found.push(m);
                        }
                    }
                    if name == crate::kw::MOD {
                        u.modulo = true;
                    }
                    if name == crate::kw::ISQRT {
                        u.root = true;
                    }
                    for a in args {
                        walk(a, found, u);
                    }
                }
                Expr::Bin(l, _, r, _) => {
                    walk(l, found, u);
                    walk(r, found, u);
                }
                _ => {}
            }
        }
        for e in exprs {
            walk(e, &mut found, &mut u);
        }
        u.div = MODES.into_iter().filter(|m| found.contains(m)).collect();
        u
    }

    pub fn any(&self) -> bool {
        !self.div.is_empty() || self.modulo || self.root
    }

    /// Every helper. The unit-vector files beside the generated code are shared by every rule
    /// generated into the directory (`python/_round_test.py` and the like) and carry their own copy
    /// of each helper, so they hold all of them to the reference whatever the rules call: written
    /// with one rule's uses, the last rule generated into the directory decided what the others'
    /// helpers were tested on. Go's is the one written per package, beside the helpers the package
    /// has, and keeps the rule's own uses.
    pub fn every() -> Uses {
        Uses { div: MODES.to_vec(), modulo: true, root: true }
    }
}

/// The Python helper of a quotient rounded with `m`: `_div_round_half` and the like. The other
/// languages read the Python spelling back (`go_expr` and the rest), as they do the five modes'.
pub fn py_div(m: RoundMode) -> String {
    format!("_div_round_{}", mode_fn(m))
}

/// The largest whole number whose square fits in int64: where the search for a root starts.
pub const ROOT_TOP: i64 = 3_037_000_499;

// ── The unit vectors ─────────────────────────────────────────────────────

const DIVIDENDS: [i64; 17] = [-25, -15, -11, -10, -7, -5, -1, 0, 1, 5, 7, 10, 11, 15, 25, i64::MAX, -i64::MAX];
const DIVISORS: [i64; 9] = [1, 2, -2, 3, 10, -10, 4, i64::MAX, -i64::MAX];
const MOD_VALUES: [i64; 12] = [-13, -7, -6, -1, 0, 1, 5, 6, 7, 13, i64::MAX, -i64::MAX];
const MOD_BY: [i64; 5] = [1, 6, 7, 1000, i64::MAX];
const ROOTS: [i64; 24] = [
    0,
    1,
    2,
    3,
    4,
    5,
    8,
    9,
    10,
    15,
    16,
    17,
    24,
    25,
    26,
    99,
    100,
    101,
    2_147_483_647,
    4_611_686_014_132_420_609,
    9_223_372_030_926_249_000,
    9_223_372_030_926_249_001,
    9_223_372_030_926_249_002,
    i64::MAX,
];

/// One unit vector: which helper, its arguments, and the answer the reference arithmetic gives.
#[derive(Clone, Debug)]
pub enum Case {
    Div(RoundMode, i64, i64, i64),
    Mod(i64, i64, i64),
    Root(i64, i64),
}

/// Every unit vector of the helpers `u` names, the answers worked out with the reference
/// evaluator's own rationals.
pub fn cases(u: &Uses) -> Vec<Case> {
    let mut out = Vec::new();
    for m in &u.div {
        for n in DIVIDENDS {
            for d in DIVISORS {
                let q = Rat::new(n as i128, d as i128).round_to(*m, Rat::int(1));
                out.push(Case::Div(*m, n, d, (q.num / q.den) as i64));
            }
        }
    }
    if u.modulo {
        for x in MOD_VALUES {
            for n in MOD_BY {
                let r = crate::types::mod_rat(Rat::int(x as i128), n as i128);
                out.push(Case::Mod(x, n, r.num as i64));
            }
        }
    }
    if u.root {
        for x in ROOTS {
            let r = crate::types::isqrt_rat(Rat::int(x as i128)).map(|r| r.num as i64).unwrap_or(0);
            out.push(Case::Root(x, r));
        }
    }
    out
}

/// How one case is called in a language and written in its message: the call with each helper's
/// name `name` gives it, and each integer as `lit` writes it.
fn call(c: &Case, name: &dyn Fn(&Case) -> String, lit: &dyn Fn(i64) -> String) -> (String, i64) {
    match c {
        Case::Div(_, n, d, want) => (format!("{}({}, {})", name(c), lit(*n), lit(*d)), *want),
        Case::Mod(x, n, want) => (format!("{}({}, {})", name(c), lit(*x), lit(*n)), *want),
        Case::Root(x, want) => (format!("{}({})", name(c), lit(*x)), *want),
    }
}

/// The helper a case calls, in the Python spelling.
fn py_name(c: &Case) -> String {
    match c {
        Case::Div(m, ..) => py_div(*m),
        Case::Mod(..) => "_mod".into(),
        Case::Root(..) => "_isqrt".into(),
    }
}

/// A helper's name in the camel case Go, TypeScript, Swift and Java write it with.
/// A helper's name in a language that writes its own in lower camel case beside `roundHalf`:
/// `divRoundHalf`, with `prefix` in front where the language's helpers carry one (`_divRoundHalf`).
fn camel_name(c: &Case, prefix: &str, md: &str, root: &str) -> String {
    match c {
        Case::Div(m, ..) => {
            let p = super::pascal(&format!("div_round_{}", mode_fn(*m)));
            let mut cs = p.chars();
            let first = cs.next().map(|f| f.to_ascii_lowercase()).unwrap_or_default();
            format!("{prefix}{first}{}", cs.as_str())
        }
        Case::Mod(..) => md.to_string(),
        Case::Root(..) => root.to_string(),
    }
}

/// What the doc comment of a quotient helper says.
fn div_doc(m: RoundMode) -> String {
    match m {
        RoundMode::Down => tr!("n / d を 0 へ寄せた整数。d は 0 でない。", "n / d toward zero, as a whole number. d is not zero."),
        RoundMode::Up => tr!("n / d を 0 から遠ざけた整数。", "n / d away from zero, as a whole number."),
        RoundMode::Half => tr!("n / d の半分ちょうどを 0 から遠ざけた整数。", "n / d with an exact half away from zero, as a whole number."),
        RoundMode::HalfDown => tr!("n / d の半分ちょうどを 0 へ寄せた整数。", "n / d with an exact half toward zero, as a whole number."),
        RoundMode::Bankers => tr!("n / d の半分ちょうどを偶数へ寄せた整数。", "n / d with an exact half to the even neighbor, as a whole number."),
    }
}

fn mod_doc() -> String {
    tr!("x を n（正）で割った余り。0 から n − 1 まで。", "The remainder of x by n (positive), from 0 to n − 1.")
}

fn root_doc() -> String {
    tr!("2 乗が x を超えない最大の整数。x は負でない。", "The largest whole number whose square does not pass x. x is not negative.")
}

/// When the rounding steps the quotient up by one: the comparison of the remainder `r` with what
/// is left to the divisor `d`, written so that nothing doubles past int64 (`r >= d - r` for `2r >=
/// d`), in each language's spelling of `||`, `&&`, `==` and the parity of `q`.
fn steps_up(m: RoundMode, or: &str, and: &str, eq: &str, odd: &str) -> Option<String> {
    Some(match m {
        RoundMode::Down => return None,
        RoundMode::Up => format!("r {} 0", if eq == "===" { "!==" } else { "!=" }),
        RoundMode::Half => "r >= d - r".into(),
        RoundMode::HalfDown => "r > d - r".into(),
        RoundMode::Bankers => format!("r > d - r {or} (r {eq} d - r {and} {odd})"),
    })
}

// ── Python ───────────────────────────────────────────────────────────────

/// The Python helpers, each `def` two blank lines from the next, the first one blank line after
/// the five modes (which end with one) and the code after them two blank lines away: the shape
/// PEP 8 asks of top-level functions, in the module and in `_round_test.py` alike.
pub fn py_helpers(u: &Uses) -> String {
    let mut fs: Vec<String> = Vec::new();
    for m in &u.div {
        let up = steps_up(*m, "or", "and", "==", "q % 2 == 1");
        let mut f = format!("def {}(n: int, d: int) -> int:\n    \"\"\"{}\"\"\"\n    if d < 0:\n        n, d = -n, -d\n", py_div(*m), div_doc(*m));
        match up {
            None => f.push_str("    q = abs(n) // d\n"),
            Some(c) => f.push_str(&format!("    q, r = abs(n) // d, abs(n) % d\n    if {c}:\n        q += 1\n")),
        }
        f.push_str("    return -q if n < 0 else q\n");
        fs.push(f);
    }
    if u.modulo {
        fs.push(format!("def _mod(x: int, n: int) -> int:\n    \"\"\"{}\"\"\"\n    return x % n\n", mod_doc()));
    }
    if u.root {
        fs.push(format!(
            "def _isqrt(x: int) -> int:\n    \"\"\"{}\"\"\"\n    lo, hi = 0, x if x < {ROOT_TOP} else {ROOT_TOP}\n    while lo < hi:\n        mid = lo + (hi - lo + 1) // 2\n        if mid * mid <= x:\n            lo = mid\n        else:\n            hi = mid - 1\n    return lo\n",
            root_doc()
        ));
    }
    if fs.is_empty() {
        return String::new();
    }
    format!("\n{}\n", fs.join("\n\n"))
}

/// The unit vectors in Python: the lines that check them, counting into `bad`, and how many there
/// are.
pub fn py_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("\nARITH = [\n");
    for c in &cs {
        let (text, want) = call(c, &py_name, &|v| v.to_string());
        o.push_str(&format!("    (\"{text}\", {text}, {want}),\n"));
    }
    o.push_str("]\n\nfor what, got, want in ARITH:\n    if got != want:\n        print(f\"NG {what} = {got}, want {want}\")\n        bad += 1\n");
    (o, cs.len())
}

// ── Go ───────────────────────────────────────────────────────────────────

fn go_name(c: &Case) -> String {
    camel_name(c, "", "modFloor", "isqrtInt")
}

pub fn go_helpers(u: &Uses) -> String {
    let mut o = String::new();
    for m in &u.div {
        let name = go_name(&Case::Div(*m, 0, 0, 0));
        let up = steps_up(*m, "||", "&&", "==", "q%2 == 1").map(|c| c.replace("d - r", "d-r"));
        o.push_str(&format!("// {name}: {}\nfunc {name}(n, d int64) int64 {{\n\tif d < 0 {{\n\t\tn, d = -n, -d\n\t}}\n", div_doc(*m)));
        match up {
            None => o.push_str("\tq, _, neg := absMod(n, d)\n"),
            Some(c) => o.push_str(&format!("\tq, r, neg := absMod(n, d)\n\tif {c} {{\n\t\tq++\n\t}}\n")),
        }
        o.push_str("\tif neg {\n\t\treturn -q\n\t}\n\treturn q\n}\n\n");
    }
    if u.modulo {
        o.push_str(&format!("// modFloor: {}\nfunc modFloor(x, n int64) int64 {{\n\tr := x % n\n\tif r < 0 {{\n\t\tr += n\n\t}}\n\treturn r\n}}\n\n", mod_doc()));
    }
    if u.root {
        o.push_str(&format!(
            "// isqrtInt: {}\nfunc isqrtInt(x int64) int64 {{\n\tlo, hi := int64(0), x\n\tif hi > {ROOT_TOP} {{\n\t\thi = {ROOT_TOP}\n\t}}\n\tfor lo < hi {{\n\t\tmid := lo + (hi-lo+1)/2\n\t\tif mid*mid <= x {{\n\t\t\tlo = mid\n\t\t}} else {{\n\t\t\thi = mid - 1\n\t\t}}\n\t}}\n\treturn lo\n}}\n\n",
            root_doc()
        ));
    }
    o
}

/// The Go spelling of the helpers, read back from the Python text `Gen::expr` writes. Before the
/// five modes' replacements: `_round_half(` is inside `_div_round_half(`.
pub fn go_expr(s: &str) -> String {
    let mut t = s.to_string();
    for m in [RoundMode::HalfDown, RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::Bankers] {
        t = t.replace(&format!("{}(", py_div(m)), &format!("{}(", go_name(&Case::Div(m, 0, 0, 0))));
    }
    t.replace("_mod(", "modFloor(").replace("_isqrt(", "isqrtInt(")
}

pub fn go_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("\nfunc TestArithmetic(t *testing.T) {\n\tcases := []struct {\n\t\twhat\x1fstring\n\t\tgot, want\x1fint64\n\t}{\n");
    for c in &cs {
        let (text, want) = call(c, &go_name, &|v| v.to_string());
        o.push_str(&format!("\t\t{{{text:?}, {text}, {want}}},\n"));
    }
    o.push_str("\t}\n\tfor _, c := range cases {\n\t\tif c.got != c.want {\n\t\t\tt.Errorf(\"%s = %d, want %d\", c.what, c.got, c.want)\n\t\t}\n\t}\n}\n");
    (o, cs.len())
}

// ── TypeScript and JavaScript ────────────────────────────────────────────

fn ts_name(c: &Case) -> String {
    camel_name(c, "_", "_mod", "_isqrt")
}

pub fn ts_helpers(u: &Uses) -> String {
    let mut o = String::new();
    for m in &u.div {
        let name = ts_name(&Case::Div(*m, 0, 0, 0));
        let up = steps_up(*m, "||", "&&", "===", "q % 2n === 1n").map(|c| c.replace(" 0", " 0n"));
        o.push_str(&format!(
            "\n/** {} */\nfunction {name}(n: bigint, d: bigint): bigint {{\n  if (d < 0n) {{\n    n = -n;\n    d = -d;\n  }}\n  const a = n < 0n ? -n : n;\n",
            div_doc(*m)
        ));
        match up {
            None => o.push_str("  const q = a / d;\n"),
            Some(c) => o.push_str(&format!("  let q = a / d;\n  const r = a % d;\n  if ({c}) {{\n    q += 1n;\n  }}\n")),
        }
        o.push_str("  return n < 0n ? -q : q;\n}\n");
    }
    if u.modulo {
        o.push_str(&format!("\n/** {} */\nfunction _mod(x: bigint, n: bigint): bigint {{\n  const r = x % n;\n  return r < 0n ? r + n : r;\n}}\n", mod_doc()));
    }
    if u.root {
        o.push_str(&format!(
            "\n/** {} */\nfunction _isqrt(x: bigint): bigint {{\n  let lo = 0n;\n  let hi = x < {ROOT_TOP}n ? x : {ROOT_TOP}n;\n  while (lo < hi) {{\n    const mid = lo + (hi - lo + 1n) / 2n;\n    if (mid * mid <= x) {{\n      lo = mid;\n    }} else {{\n      hi = mid - 1n;\n    }}\n  }}\n  return lo;\n}}\n",
            root_doc()
        ));
    }
    o
}

pub fn ts_expr(s: &str) -> String {
    let mut t = s.to_string();
    for m in [RoundMode::HalfDown, RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::Bankers] {
        t = t.replace(&format!("{}(", py_div(m)), &format!("{}(", ts_name(&Case::Div(m, 0, 0, 0))));
    }
    t
}

pub fn ts_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("\nconst ARITH: [string, bigint, bigint][] = [\n");
    for c in &cs {
        let (text, want) = call(c, &ts_name, &|v| format!("{v}n"));
        o.push_str(&format!("  [{text:?}, {text}, {want}n],\n"));
    }
    o.push_str("];\nfor (const [what, got, want] of ARITH) {\n  if (got !== want) {\n    console.log(`NG ${what} = ${got}, want ${want}`);\n    bad += 1;\n  }\n}\n");
    (o, cs.len())
}

// ── Rust (and the Wasm built from it) ───────────────────────────────────

/// The Rust helpers are named `quot_*`: a `div_round_*` would hold `_round_half(` and the like,
/// which `rs_expr` turns into the five modes' names.
fn rs_name(c: &Case) -> String {
    match c {
        Case::Div(m, ..) => format!("quot_{}", mode_fn(*m)),
        Case::Mod(..) => "mod_floor".into(),
        Case::Root(..) => "isqrt_i64".into(),
    }
}

pub fn rs_helpers(u: &Uses) -> String {
    let mut o = String::new();
    for m in &u.div {
        let name = rs_name(&Case::Div(*m, 0, 0, 0));
        let up = steps_up(*m, "||", "&&", "==", "q % 2 == 1");
        o.push_str(&format!(
            "\n/// {}\npub(crate) fn {name}(n: i64, d: i64) -> i64 {{\n    let (n, d) = if d < 0 {{ (-n, -d) }} else {{ (n, d) }};\n",
            div_doc(*m)
        ));
        match up {
            None => o.push_str("    let q = n.abs() / d;\n"),
            Some(c) => o.push_str(&format!("    let (mut q, r) = (n.abs() / d, n.abs() % d);\n    if {c} {{\n        q += 1;\n    }}\n")),
        }
        o.push_str("    if n < 0 { -q } else { q }\n}\n");
    }
    if u.modulo {
        o.push_str(&format!("\n/// {}\npub(crate) fn mod_floor(x: i64, n: i64) -> i64 {{\n    x.rem_euclid(n)\n}}\n", mod_doc()));
    }
    if u.root {
        o.push_str(&format!(
            "\n/// {}\npub(crate) fn isqrt_i64(x: i64) -> i64 {{\n    let (mut lo, mut hi) = (0i64, x.min({ROOT_TOP}));\n    while lo < hi {{\n        let mid = lo + (hi - lo + 1) / 2;\n        if mid * mid <= x {{\n            lo = mid;\n        }} else {{\n            hi = mid - 1;\n        }}\n    }}\n    lo\n}}\n",
            root_doc()
        ));
    }
    o
}

pub fn rs_expr(s: &str) -> String {
    let mut t = s.to_string();
    for m in [RoundMode::HalfDown, RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::Bankers] {
        t = t.replace(&format!("{}(", py_div(m)), &format!("{}(", rs_name(&Case::Div(m, 0, 0, 0))));
    }
    t.replace("_mod(", "mod_floor(").replace("_isqrt(", "isqrt_i64(")
}

/// The unit vectors in Rust, inside `main`, counting into `bad`.
pub fn rs_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("    let arith: Vec<(&str, i64, i64)> = vec![\n");
    for c in &cs {
        let (text, want) = call(c, &rs_name, &|v| v.to_string());
        o.push_str(&format!("        ({text:?}, {text}, {want}),\n"));
    }
    o.push_str("    ];\n    for (what, got, want) in &arith {\n        if got != want {\n            println!(\"NG {what} = {got}, want {want}\");\n            bad += 1;\n        }\n    }\n");
    (o, cs.len())
}

/// The exports of the Wasm module the unit vectors run through on the JavaScript side.
pub fn wasm_exports(u: &Uses) -> String {
    let mut o = String::new();
    for m in &u.div {
        let name = rs_name(&Case::Div(*m, 0, 0, 0));
        o.push_str(&format!("\n#[no_mangle]\npub extern \"C\" fn rulec_{name}(n: i64, d: i64) -> i64 {{\n    {name}(n, d)\n}}\n"));
    }
    if u.modulo {
        o.push_str("\n#[no_mangle]\npub extern \"C\" fn rulec_mod_floor(x: i64, n: i64) -> i64 {\n    mod_floor(x, n)\n}\n");
    }
    if u.root {
        o.push_str("\n#[no_mangle]\npub extern \"C\" fn rulec_isqrt_i64(x: i64) -> i64 {\n    isqrt_i64(x)\n}\n");
    }
    o
}

pub fn wasm_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("const ARITH = [\n");
    for c in &cs {
        let name = |c: &Case| format!("ex.rulec_{}", rs_name(c));
        let (text, want) = call(c, &name, &|v| format!("{v}n"));
        o.push_str(&format!("  [{text:?}, {text}, {want}n],\n"));
    }
    o.push_str("];\nfor (const [what, got, want] of ARITH) {\n  if (got !== want) {\n    console.log(`NG ${what} = ${got}, want ${want}`);\n    bad += 1;\n  }\n}\n");
    (o, cs.len())
}

// ── Ruby ─────────────────────────────────────────────────────────────────

pub fn rb_helpers(u: &Uses) -> String {
    let mut o = String::new();
    let mut names: Vec<String> = Vec::new();
    for m in &u.div {
        let name = py_div(*m);
        let up = steps_up(*m, "||", "&&", "==", "q.odd?");
        o.push_str(&format!("\n  # {}\n  def self.{name}(n, d)\n    n, d = -n, -d if d.negative?\n", div_doc(*m)));
        match up {
            None => o.push_str("    q = n.abs / d\n"),
            Some(c) => o.push_str(&format!("    q, r = n.abs / d, n.abs % d\n    q += 1 if {c}\n")),
        }
        o.push_str("    n.negative? ? -q : q\n  end\n");
        names.push(format!(":{name}"));
    }
    if u.modulo {
        o.push_str(&format!("\n  # {}\n  def self._mod(x, n)\n    x % n\n  end\n", mod_doc()));
        names.push(":_mod".into());
    }
    if u.root {
        o.push_str(&format!(
            "\n  # {}\n  def self._isqrt(x)\n    lo = 0\n    hi = x < {ROOT_TOP} ? x : {ROOT_TOP}\n    while lo < hi\n      mid = lo + ((hi - lo + 1) / 2)\n      if mid * mid <= x\n        lo = mid\n      else\n        hi = mid - 1\n      end\n    end\n    lo\n  end\n",
            root_doc()
        ));
        names.push(":_isqrt".into());
    }
    if !names.is_empty() {
        o.push_str(&format!("\n  private_class_method {}\n", names.join(", ")));
    }
    o
}

/// The names `public_class_method` opens in the unit vectors' file.
pub fn rb_names(u: &Uses) -> Vec<String> {
    let mut v: Vec<String> = u.div.iter().map(|m| format!(":{}", py_div(*m))).collect();
    if u.modulo {
        v.push(":_mod".into());
    }
    if u.root {
        v.push(":_isqrt".into());
    }
    v
}

pub fn rb_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("\nARITH = [\n");
    for c in &cs {
        let name = |c: &Case| format!("R.{}", py_name(c));
        let (text, want) = call(c, &name, &|v| v.to_string());
        o.push_str(&format!("  [{text:?}, {text}, {want}],\n"));
    }
    o.push_str("].freeze\n\nARITH.each do |what, got, want|\n  next if got == want\n\n  puts \"NG #{what} = #{got}, want #{want}\"\n  bad += 1\nend\n");
    (o, cs.len())
}

// ── Swift ────────────────────────────────────────────────────────────────

fn sw_name(c: &Case) -> String {
    camel_name(c, "_", "_mod", "_isqrt")
}

pub fn sw_helpers(u: &Uses) -> String {
    let mut o = String::new();
    for m in &u.div {
        let name = sw_name(&Case::Div(*m, 0, 0, 0));
        let up = steps_up(*m, "||", "&&", "==", "q % 2 == 1");
        o.push_str(&format!(
            "\n/// {}\nprivate func {name}(_ n0: Int64, _ d0: Int64) -> Int64 {{\n    let (n, d) = d0 < 0 ? (-n0, -d0) : (n0, d0)\n    let a = abs(n)\n",
            div_doc(*m)
        ));
        match up {
            None => o.push_str("    let q = a / d\n"),
            Some(c) => o.push_str(&format!("    var q = a / d\n    let r = a % d\n    if {c} {{\n        q += 1\n    }}\n")),
        }
        o.push_str("    return n < 0 ? -q : q\n}\n");
    }
    if u.modulo {
        o.push_str(&format!("\n/// {}\nprivate func _mod(_ x: Int64, _ n: Int64) -> Int64 {{\n    let r = x % n\n    return r < 0 ? r + n : r\n}}\n", mod_doc()));
    }
    if u.root {
        o.push_str(&format!(
            "\n/// {}\nprivate func _isqrt(_ x: Int64) -> Int64 {{\n    var lo: Int64 = 0\n    var hi: Int64 = x < {ROOT_TOP} ? x : {ROOT_TOP}\n    while lo < hi {{\n        let mid = lo + (hi - lo + 1) / 2\n        if mid * mid <= x {{\n            lo = mid\n        }} else {{\n            hi = mid - 1\n        }}\n    }}\n    return lo\n}}\n",
            root_doc()
        ));
    }
    o
}

pub fn sw_expr(s: &str) -> String {
    ts_expr(s)
}

pub fn sw_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("let arith: [(String, Int64, Int64)] = [\n");
    for c in &cs {
        let (text, want) = call(c, &sw_name, &|v| v.to_string());
        o.push_str(&format!("    ({text:?}, {text}, {want}),\n"));
    }
    o.push_str("]\nfor (what, got, want) in arith where got != want {\n    print(\"NG \\(what) = \\(got), want \\(want)\")\n    bad += 1\n}\n");
    (o, cs.len())
}

// ── PHP ──────────────────────────────────────────────────────────────────

pub fn php_helpers(u: &Uses) -> String {
    let mut o = String::new();
    for m in &u.div {
        let name = py_div(*m);
        let up = match m {
            RoundMode::Down => None,
            RoundMode::Up => Some("$r !== 0".to_string()),
            RoundMode::Half => Some("$r >= $d - $r".to_string()),
            RoundMode::HalfDown => Some("$r > $d - $r".to_string()),
            RoundMode::Bankers => Some("$r > $d - $r || ($r === $d - $r && $q % 2 === 1)".to_string()),
        };
        o.push_str(&format!(
            "\n/** {} */\nfunction {name}(int $n, int $d): int\n{{\n    if ($d < 0) {{\n        $n = -$n;\n        $d = -$d;\n    }}\n",
            div_doc(*m)
        ));
        match up {
            None => o.push_str("    $q = intdiv(abs($n), $d);\n"),
            Some(c) => o.push_str(&format!("    $q = intdiv(abs($n), $d);\n    $r = abs($n) % $d;\n    if ({c}) {{\n        $q += 1;\n    }}\n")),
        }
        o.push_str("    return $n < 0 ? -$q : $q;\n}\n");
    }
    if u.modulo {
        o.push_str(&format!("\n/** {} */\nfunction _mod(int $x, int $n): int\n{{\n    $r = $x % $n;\n    return $r < 0 ? $r + $n : $r;\n}}\n", mod_doc()));
    }
    if u.root {
        o.push_str(&format!(
            "\n/** {} */\nfunction _isqrt(int $x): int\n{{\n    $lo = 0;\n    $hi = $x < {ROOT_TOP} ? $x : {ROOT_TOP};\n    while ($lo < $hi) {{\n        $mid = $lo + intdiv($hi - $lo + 1, 2);\n        if ($mid * $mid <= $x) {{\n            $lo = $mid;\n        }} else {{\n            $hi = $mid - 1;\n        }}\n    }}\n    return $lo;\n}}\n",
            root_doc()
        ));
    }
    o
}

pub fn php_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("\n$arith = [\n");
    for c in &cs {
        let (text, want) = call(c, &py_name, &|v| if v == -i64::MAX { "-PHP_INT_MAX".into() } else { v.to_string() });
        o.push_str(&format!("    ['{}', {text}, {want}],\n", text.replace('\'', "\\'")));
    }
    o.push_str("];\nforeach ($arith as [$what, $got, $want]) {\n    if ($got === $want) {\n        continue;\n    }\n    echo \"NG {$what} = {$got}, want {$want}\\n\";\n    $bad++;\n}\n");
    (o, cs.len())
}

// ── Java ─────────────────────────────────────────────────────────────────

fn java_name(c: &Case) -> String {
    camel_name(c, "", "Math.floorMod", "isqrtLong")
}

pub fn java_helpers(u: &Uses) -> String {
    let mut o = String::new();
    for m in &u.div {
        let name = java_name(&Case::Div(*m, 0, 0, 0));
        let up = steps_up(*m, "||", "&&", "==", "q % 2 == 1");
        o.push_str(&format!(
            "\n/** {} */\nprivate static long {name}(long n, long d) {{\n    if (d < 0) {{\n        n = -n;\n        d = -d;\n    }}\n    long q = Math.abs(n) / d;\n",
            div_doc(*m)
        ));
        if let Some(c) = up {
            o.push_str(&format!("    long r = Math.abs(n) % d;\n    if ({c}) {{\n        q += 1;\n    }}\n"));
        }
        o.push_str("    return n < 0 ? -q : q;\n}\n");
    }
    if u.root {
        o.push_str(&format!(
            "\n/** {} */\nprivate static long isqrtLong(long x) {{\n    long lo = 0;\n    long hi = x < {ROOT_TOP}L ? x : {ROOT_TOP}L;\n    while (lo < hi) {{\n        long mid = lo + (hi - lo + 1) / 2;\n        if (mid * mid <= x) {{\n            lo = mid;\n        }} else {{\n            hi = mid - 1;\n        }}\n    }}\n    return lo;\n}}\n",
            root_doc()
        ));
    }
    o
}

pub fn java_expr(s: &str) -> String {
    let mut t = s.to_string();
    for m in [RoundMode::HalfDown, RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::Bankers] {
        t = t.replace(&format!("{}(", py_div(m)), &format!("{}(", java_name(&Case::Div(m, 0, 0, 0))));
    }
    t.replace("_mod(", "Math.floorMod(").replace("_isqrt(", "isqrtLong(")
}

pub fn java_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("        Object[][] arith = {\n");
    for c in &cs {
        let (text, want) = call(c, &java_name, &|v| format!("{v}L"));
        o.push_str(&format!("            {{{text:?}, {text}, {want}L}},\n"));
    }
    o.push_str("        };\n        for (Object[] a : arith) {\n            if (((Long) a[1]).longValue() != ((Long) a[2]).longValue()) {\n                System.out.println(\"NG \" + a[0] + \" = \" + a[1] + \", want \" + a[2]);\n                bad++;\n            }\n        }\n");
    (o, cs.len())
}

// ── SQL ──────────────────────────────────────────────────────────────────

/// A rounded quotient as SQL arithmetic over two bound columns `n` and `d`: the magnitudes divided,
/// stepped by the remainder, and given the sign of the two. Integer division and `%` truncate
/// toward zero in PostgreSQL and SQLite alike, which on two magnitudes is the floor.
///
/// The divisor is never zero on an input the door takes (E132); a refused row is decided all the
/// same, and PostgreSQL raises on a division by zero where SQLite answers NULL, so the magnitude
/// of the divisor goes through NULLIF and both answer NULL there.
pub fn sql_div(m: RoundMode, n: &str, d: &str) -> String {
    let (a, b) = (format!("ABS({n})"), format!("NULLIF(ABS({d}), 0)"));
    let sign = format!("(CASE WHEN ({n} < 0) <> ({d} < 0) THEN -1 ELSE 1 END)");
    let q = format!("({a} / {b})");
    let r = format!("({a} % {b})");
    let cond = match m {
        RoundMode::Down => return format!("({sign} * {q})"),
        RoundMode::Up => format!("{r} <> 0"),
        RoundMode::Half => format!("{r} >= {b} - {r}"),
        RoundMode::HalfDown => format!("{r} > {b} - {r}"),
        RoundMode::Bankers => format!("{r} > {b} - {r} OR ({r} = {b} - {r} AND {q} % 2 = 1)"),
    };
    format!("({sign} * (CASE WHEN {cond} THEN {q} + 1 ELSE {q} END))")
}

/// The remainder by a positive constant, over a bound column.
pub fn sql_mod(x: &str, n: &str) -> String {
    format!("(CASE WHEN {x} % {n} < 0 THEN {x} % {n} + {n} ELSE {x} % {n} END)")
}

/// The whole square root: a function, since a search does not fit in one expression. PostgreSQL
/// gets it as PL/pgSQL beside the query; the SQLite the runner holds the query to registers the
/// same search under the same name.
pub const SQL_ROOT: &str = "rulec_isqrt";

pub fn sql_root_function() -> String {
    format!(
        "CREATE OR REPLACE FUNCTION {SQL_ROOT}(x BIGINT)\nRETURNS BIGINT\nLANGUAGE plpgsql IMMUTABLE STRICT PARALLEL SAFE AS $isqrt$\nDECLARE\n  lo BIGINT := 0;\n  hi BIGINT := LEAST(x, {ROOT_TOP});\n  mid BIGINT;\nBEGIN\n  WHILE lo < hi LOOP\n    mid := lo + (hi - lo + 1) / 2;\n    IF mid * mid <= x THEN\n      lo := mid;\n    ELSE\n      hi := mid - 1;\n    END IF;\n  END LOOP;\n  RETURN lo;\nEND;\n$isqrt$;\n"
    )
}

/// The same search in Python, which the SQLite runner registers as `rulec_isqrt`. A NULL is passed
/// over, as the PostgreSQL function (`STRICT`) passes it: a row the door refuses is decided all
/// the same, and a value outside an enum leaves one (§15.204).
pub fn sql_root_python() -> String {
    format!(
        "def _rulec_isqrt(x):\n    if x is None:\n        return None\n    lo, hi = 0, x if x < {ROOT_TOP} else {ROOT_TOP}\n    while lo < hi:\n        mid = lo + (hi - lo + 1) // 2\n        if mid * mid <= x:\n            lo = mid\n        else:\n            hi = mid - 1\n    return lo\n\n\n"
    )
}

pub fn sql_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::new();
    if u.root {
        o.push_str(&format!("{}db.create_function(\"{SQL_ROOT}\", 1, _rulec_isqrt, deterministic=True)\n", sql_root_python()));
    }
    o.push_str("ARITH = [\n");
    for c in &cs {
        let (sql, what, want) = match c {
            Case::Div(m, n, d, want) => (sql_div(*m, &format!("({n})"), &format!("({d})")), format!("{}({n}, {d})", py_div(*m)), *want),
            Case::Mod(x, n, want) => (sql_mod(&format!("({x})"), &n.to_string()), format!("_mod({x}, {n})"), *want),
            Case::Root(x, want) => (format!("{SQL_ROOT}({x})"), format!("_isqrt({x})"), *want),
        };
        o.push_str(&format!("    ({what:?}, {:?}, {want}),\n", sql));
    }
    o.push_str("]\nfor what, sql, want in ARITH:\n    got = db.execute(\"SELECT \" + sql).fetchone()[0]\n    if got != want:\n        print(f\"NG {what} = {got}, want {want}\")\n        bad += 1\n");
    (o, cs.len())
}

// ── NumPy ────────────────────────────────────────────────────────────────

pub fn np_tests(u: &Uses) -> (String, usize) {
    let cs = cases(u);
    if cs.is_empty() {
        return (String::new(), 0);
    }
    let mut o = String::from("ARITH = [\n");
    for c in &cs {
        let one = |v: i64| format!("np.array([{v}], dtype=np.int64)");
        let (what, call, want) = match c {
            Case::Div(m, n, d, want) => (format!("{}({n}, {d})", py_div(*m)), format!("rulec_np._div_round({:?}, {}, {})", m.name(), one(*n), one(*d)), *want),
            Case::Mod(x, n, want) => (format!("_mod({x}, {n})"), format!("rulec_np._mod({}, np.int64({n}))", one(*x)), *want),
            Case::Root(x, want) => (format!("_isqrt({x})"), format!("rulec_np._isqrt({})", one(*x)), *want),
        };
        o.push_str(&format!("    ({what:?}, int({call}[0]), {want}),\n"));
    }
    o.push_str("]\nfor what, got, want in ARITH:\n    if got != want:\n        print(f\"NG {what} = {got}, want {want}\")\n        bad += 1\n");
    (o, cs.len())
}
