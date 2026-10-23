//! PHP, the tenth target (§15.77).
//!
//! The same argument that put Ruby fifth (§15.20), one installed base larger: a fee table, a
//! discount, an eligibility test in a Japanese shop is very often a PHP function, inside
//! WooCommerce or EC-CUBE or Laravel. It passes the gate of §15.13 with nothing added —
//! `php` is one binary, `ext/json` is compiled in and cannot be switched off since 8.0, so
//! the runner needs no composer and no extension.
//!
//! Three things are different from Ruby, and each one is a decision:
//!
//! **The types are written down.** PHP declares scalar and enum parameter types, and
//! `declare(strict_types=1)` makes them refuse a float where an int was asked for. So the
//! entry guard is the one Go, Rust and Swift emit — the range only — and not the one Python,
//! TypeScript and Ruby emit, which also has to ask whether the value is an integer at all.
//! What PHP cannot carry is the unit; that is documented, as it is in Ruby (§15.20).
//!
//! **Nothing is divided with `/`.** `/` returns a float the moment the division is not
//! exact, and a float silently loses the low digits of a yen amount above 2^53 — the failure
//! this tool exists to prevent, at the one place the proof cannot see. Every division is
//! `intdiv`, which truncates toward zero exactly as Go, Rust, Swift and the `bigint` of
//! TypeScript do. The same trap was measured in Rego (§15.75), where `(7/2)*2` comes back 7.
//!
//! **An integer that overflows becomes a float**, rather than wrapping as it does in Go and
//! Rust. E108 proves no intermediate leaves int64, so this cannot be reached from a rule that
//! passed `check`; it is named here because the reader of the generated code deserves to know
//! which side of the proof the language sits on.

use super::{
    cell_src, civil_needed, fired_doc, mode_fn, out_src, pascal, plain_doc, pub_name,
    raw_base, record_doc, round_cases, traced_doc, unparen, walked_doc, wire_of, Expr2, Gen, Lang, Phase, WalkStep, Wire,
};
use crate::ast::{Arm, Cell, CmpOp, Item, Lit, OutCell, Table};
use crate::num::{Rat, RoundMode};
use crate::types::Ty;
use std::collections::BTreeMap;

/// The words PHP will not accept as a function, class or constant name. They are matched
/// without regard to case, because PHP's keywords are case-insensitive: `List` is as
/// reserved as `list`.
const PHP_KEYWORDS: &[&str] = &[
    "abstract", "and", "array", "as", "break", "callable", "case", "catch", "class", "clone",
    "const", "continue", "declare", "default", "die", "do", "echo", "else", "elseif", "empty",
    "enddeclare", "endfor", "endforeach", "endif", "endswitch", "endwhile", "enum", "eval",
    "exit", "extends", "final", "finally", "fn", "for", "foreach", "function", "global", "goto",
    "if", "implements", "include", "include_once", "instanceof", "insteadof", "interface",
    "isset", "list", "match", "namespace", "new", "or", "print", "private", "protected",
    "public", "readonly", "require", "require_once", "return", "static", "switch", "throw",
    "trait", "try", "unset", "use", "var", "while", "xor", "yield", "self", "parent", "true",
    "false", "null", "void", "never", "iterable", "object", "mixed", "int", "float", "string",
    "bool",
];

/// A name PHP will accept where a function, class or constant is declared. A keyword takes a
/// trailing `_` — there is no backtick escape as there is in Swift (§15.24) — and a name that
/// starts with a digit takes a leading one.
pub(super) fn php_name(s: &str) -> String {
    let mut n = s.to_string();
    if n.starts_with(|c: char| c.is_ascii_digit()) {
        n.insert(0, '_');
    }
    if PHP_KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(&n)) {
        n.push('_');
    }
    n
}

/// The same for a local variable. `$this` cannot be assigned even outside a class, and the
/// superglobals would be silently shared with the rest of the process, so those move too.
/// Keywords do not: `$list` is a perfectly good variable.
fn php_var(s: &str) -> String {
    const TAKEN: &[&str] = &[
        "this", "GLOBALS", "_SERVER", "_GET", "_POST", "_FILES", "_COOKIE", "_SESSION",
        "_REQUEST", "_ENV",
    ];
    let mut n = s.to_string();
    if n.starts_with(|c: char| c.is_ascii_digit()) {
        n.insert(0, '_');
    }
    if TAKEN.contains(&n.as_str()) {
        n.push('_');
    }
    format!("${n}")
}

/// The same, for the places outside this file that have to spell a variable the way the
/// generated PHP spells it — `rulec api`, whose entry a caller reads instead of the code.
pub(super) fn php_var_pub(s: &str) -> String {
    php_var(s)
}

/// A property of `Output` or of an element: the promoted parameter of its constructor, so the
/// variable's name without its `$`, read as it is declared (§15.263). A keyword is a property's name
/// as it stands (`$out->list`); `$this` and the superglobals are not a parameter's, and take `_`.
pub(super) fn php_prop(s: &str) -> String {
    php_var(s).trim_start_matches('$').to_string()
}

/// A string literal. Single quotes, so that a `$` inside a table name or a label is a
/// dollar sign and not the start of an interpolation.
pub(super) fn php_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// `(a // b)` → `intdiv(a, b)`.
///
/// `Gen::expr` writes integer division in Python's spelling and always inside its own
/// parentheses, so the operands are the two halves of the group the operator sits in. Every
/// other backend replaces ` // ` with ` / ` and lets the language divide; PHP has no integer
/// division operator at all, and its `/` would hand back a float.
///
/// **The group is not always there.** An expression that *is* one division reaches here
/// through `unparen`, which takes the outer pair off — so the operator sits at the top level
/// and its operands are the whole text. Giving up in that case left `… // 20000;` in the
/// file, which PHP reads as a comment and a statement with no terminator; `tests/apply.rs`
/// found it on the one corpus shape whose callee output is rescaled.
fn php_div(s: &str) -> String {
    let mut out = s.to_string();
    while let Some(op) = out.find(" // ") {
        let b = out.as_bytes();
        // The '(' that opens the group this operator sits in. Only ASCII bytes are compared,
        // and a UTF-8 continuation byte is never one of them, so walking bytes is safe.
        let mut depth = 0i32;
        let mut i = op;
        let open = loop {
            if i == 0 {
                break None;
            }
            i -= 1;
            match b[i] {
                b')' => depth += 1,
                b'(' if depth == 0 => break Some(i),
                b'(' => depth -= 1,
                _ => {}
            }
        };
        let mut depth = 0i32;
        let mut j = op + 4;
        let close = loop {
            if j >= b.len() {
                break None;
            }
            match b[j] {
                b'(' => depth += 1,
                b')' if depth == 0 => break Some(j),
                b')' => depth -= 1,
                _ => {}
            }
            j += 1;
        };
        // No enclosing group: the operator is at the top level, so the operands run to the
        // ends of the text.
        let (head, lo) = match open {
            Some(i) => (&out[..i], i + 1),
            None => ("", 0),
        };
        let (hi, tail) = match close {
            Some(j) => (j, out[j + 1..].to_string()),
            None => (out.len(), String::new()),
        };
        let lhs = out[lo..op].to_string();
        let rhs = out[op + 4..hi].to_string();
        out = format!("{head}intdiv({lhs}, {rhs}){tail}");
    }
    out
}

/// The shared expression text as PHP.
fn php_expr(s: &str) -> String {
    php_div(&s.replace("True", "true").replace("False", "false"))
}

/// The five modes of §7.3 and the two comparisons, at the top level of the file. The
/// magnitude is rounded and the sign put back, so that a mode means the same thing here as
/// it does in the other ten targets.
fn round_php() -> String {
    format!(
        r#"
/** {down} */
function _round_down(int $x, int $g): int
{{
    $v = intdiv(abs($x), $g) * $g;
    return $x < 0 ? -$v : $v;
}}

/** {up} */
function _round_up(int $x, int $g): int
{{
    $q = intdiv(abs($x), $g);
    $r = abs($x) % $g;
    $v = $r === 0 ? $q * $g : ($q + 1) * $g;
    return $x < 0 ? -$v : $v;
}}

/** {half} */
function _round_half(int $x, int $g): int
{{
    $q = intdiv(abs($x), $g);
    $r = abs($x) % $g;
    $v = 2 * $r >= $g ? ($q + 1) * $g : $q * $g;
    return $x < 0 ? -$v : $v;
}}

/** {half_down} */
function _round_half_down(int $x, int $g): int
{{
    $q = intdiv(abs($x), $g);
    $r = abs($x) % $g;
    $v = 2 * $r > $g ? ($q + 1) * $g : $q * $g;
    return $x < 0 ? -$v : $v;
}}

/** {bankers} */
function _round_bankers(int $x, int $g): int
{{
    $q = intdiv(abs($x), $g);
    $r = abs($x) % $g;
    if (2 * $r > $g || (2 * $r === $g && $q % 2 === 1)) {{
        $q += 1;
    }}
    $v = $q * $g;
    return $x < 0 ? -$v : $v;
}}

function _min(int $a, int $b): int
{{
    return $a < $b ? $a : $b;
}}

function _max(int $a, int $b): int
{{
    return $a > $b ? $a : $b;
}}
"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen -> -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen -> -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        half_down = tr!("半分ちょうどは 0 へ寄せる。", "An exact half goes toward zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The namespace: the rule's ASCII alias in PascalCase, like the Go package and the Ruby
    /// module.
    pub(super) fn php_ns(&self) -> String {
        php_name(&pascal(&pub_name(&self.f.name)))
    }

    /// The public function's name.
    pub(super) fn php_fname(&self) -> String {
        php_name(&pub_name(&self.f.name))
    }

    /// The type PHP declares for a value. Enums, strings, booleans and integers are all
    /// expressible; the unit is not, and travels in the doc comment instead (§15.20).
    pub(super) fn php_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => php_name(&self.enum_names.get(n).cloned().unwrap_or_else(|| "string".into())),
            Ty::Bool => "bool".into(),
            Ty::Str => "string".into(),
            Ty::Opt(t) => format!("?{}", self.php_ty(t)),
            _ => "int".into(),
        }
    }

    /// How a value's type is written for a reader: the declared type, and the unit beside it
    /// where there is one.
    fn php_doc_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(_) | Ty::Bool | Ty::Str | Ty::Opt(_) => self.php_ty(ty),
            _ => format!("int  // {ty}"),
        }
    }

    /// An enum value as the case the file declares for it.
    pub(super) fn php_value(&self, v: &str, ty: &Ty) -> String {
        match self.value_name(v, ty) {
            Some((ty, alias)) => format!("{}::{}", php_name(ty), php_name(&alias.to_uppercase())),
            None => php_str(v),
        }
    }

    /// One enum as PHP: a backed enum, its cases in the order declared, and beside it the
    /// functions for its order and its display names when it has them (§15.210, §15.211).
    pub(super) fn php_enum(&self, jp: &str, ascii: &str) -> String {
        let Some(vals) = self.c.enums.get(jp) else { return String::new() };
        let cls = php_name(ascii);
        let mut o = format!("/** {jp} */\nenum {cls}: string\n{{\n");
        let names: Vec<String> = vals.iter().map(|v| php_name(&self.py_member(jp, v))).collect();
        for (v, n) in vals.iter().zip(&names) {
            o.push_str(&format!("    case {n} = {};\n", php_str(v)));
        }
        o.push_str("}\n\n");
        let chain = |o: &mut String, ret: &dyn Fn(usize, &String) -> String| {
            o.push_str("    return match ($x) {\n");
            for (i, (v, n)) in vals.iter().zip(&names).enumerate() {
                o.push_str(&format!("        {cls}::{n} => {},\n", ret(i, v)));
            }
            o.push_str("    };\n}\n\n");
        };
        if self.c.ordered.contains(jp) {
            o.push_str(&format!(
                "/** {} */\nfunction {}({cls} $x): int\n{{\n",
                tr!("x が {ascii} の順序の何番目か（0 から）。", "The place of x in the order of {ascii}, counting from 0."),
                self.php_own_fn(jp, "rank")
            ));
            chain(&mut o, &|i, _| i.to_string());
        }
        if let Some(labels) = self.c.labels.get(jp) {
            o.push_str(&format!(
                "/** {} */\nfunction {}({cls} $x): string\n{{\n",
                tr!("x の表示名。表示名の無い値は値の名前。", "The display name of x: a value without one is shown by its name."),
                self.php_own_fn(jp, "label")
            ));
            chain(&mut o, &|i, v| php_str(&labels.get(i).cloned().flatten().unwrap_or_else(|| v.clone())));
        }
        o
    }

    /// `era_rank` or `unit_name_label`, as the file that declares the enum names it.
    fn php_own_fn(&self, en: &str, what: &str) -> String {
        format!("{}_{what}", super::snake(&self.enum_type(en)))
    }

    /// The same function as this rule calls it: the one of an `enums` file by its namespace
    /// (§15.212).
    fn php_fn_of(&self, en: &str, what: &str) -> String {
        let f = self.php_own_fn(en, what);
        match self.c.owners.get(en) {
            Some(o) => format!("\\{}\\{f}", php_name(&pascal(&pub_name(&o.module)))),
            None => f,
        }
    }

    /// The same function as code outside any namespace names it, for `rulec api`.
    pub(super) fn php_fn_of_pub(&self, en: &str, what: &str) -> String {
        let f = self.php_fn_of(en, what);
        if f.starts_with('\\') { f } else { format!("\\{}\\{f}", self.php_ns()) }
    }

    /// The fully qualified name of an enum's type, for the runner and the projection, which
    /// name it from outside the rule's namespace (§15.212).
    pub(super) fn php_enum_fqn(&self, en: &str) -> String {
        let cls = php_name(&self.enum_names.get(en).cloned().unwrap_or_default());
        match self.c.owners.get(en) {
            Some(o) => format!("\\{}\\{cls}", php_name(&pascal(&pub_name(&o.module)))),
            None => format!("\\{}\\{cls}", self.php_ns()),
        }
    }

    /// The helpers the functions call for the expressions over an ordered enum (§15.210).
    fn php_enum_helpers(&self) -> String {
        let mut o = String::new();
        for en in self.c.enum_order.iter().filter(|e| self.c.ordered.contains(*e)) {
            let (rank, max, min) = self.enum_helpers(en);
            let (t, r) = (self.enum_type(en), self.php_fn_of(en, "rank"));
            let cls = php_name(&t);
            if rank {
                o.push_str(&format!("function _rank_{t}({cls} $x): int\n{{\n    return {r}($x);\n}}\n\n"));
            }
            if max {
                o.push_str(&format!("function _max_{t}({cls} $a, {cls} $b): {cls}\n{{\n    return {r}($a) >= {r}($b) ? $a : $b;\n}}\n\n"));
            }
            if min {
                o.push_str(&format!("function _min_{t}({cls} $a, {cls} $b): {cls}\n{{\n    return {r}($a) <= {r}($b) ? $a : $b;\n}}\n\n"));
            }
        }
        o
    }

    /// The constant that holds a group's members.
    fn php_group(&self, name: &str) -> String {
        format!("GROUP_{}", php_name(&self.ident(name)))
    }

    /// Render a cell as a PHP condition. A don't-care yields None (no condition).
    pub(super) fn php_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        // An optional value is `null` when it is absent: `!null` is true and `null < 5` is true
        // in PHP, so the absent value took the `false` row and every `<` row (§15.201).
        let optional = matches!(ty, Ty::Opt(_));
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.php_value(w, ty),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => php_str(s),
            }
        };
        // `in_array` with the third argument compares with `===`, which is what an enum case
        // (a singleton object), an int and a string all want.
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.php_value(m, ty)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("[{}]", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} === null"),
            Cell::Prefix(ps) => ps
                .iter()
                .map(|p| format!("str_starts_with({var}, {})", php_str(p)))
                .collect::<Vec<_>>()
                .join(" || "),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("in_array({var}, {}, true)", self.php_group(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE && optional => format!("{var} === true"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE && optional => format!("{var} === false"),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} === {}", lit(l)),
            Cell::Set(ls) => format!("in_array({var}, {}, true)", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!in_array({var}, {}, true)", self.php_group(w))
            }
            Cell::Not(ls) => format!("!in_array({var}, {}, true)", members(ls)),
            Cell::Cmp(cs) => {
                let c = cs
                    .iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        format!("{var} {op} {}", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(" && ");
                if optional { format!("{var} !== null && {c}") } else { c }
            }
            // A comparison on an ordered enum, by the places in its order (§15.210).
            Cell::Ranked { ops, .. } => {
                let en = self.c.ordered_enum(ty)?;
                let r = self.php_fn_of(en, "rank");
                let c = ops
                    .iter()
                    .map(|(o, v)| format!("{r}({var}) {} {r}({})", o.word(), self.php_value(v, ty)))
                    .collect::<Vec<_>>()
                    .join(" && ");
                if optional { format!("{var} !== null && {c}") } else { c }
            }
        })
    }

    /// The module: one file, no composer, no autoloader.
    pub fn php(&self) -> String {
        let mut o = String::from("<?php\n\n");
        o.push_str(&self.header("//"));
        o.push_str("\ndeclare(strict_types=1);\n\n");
        o.push_str(&format!("namespace {};\n\n", self.php_ns()));

        // The file of each `enums` file the rule reads its types from, beside it, and the
        // types by their names here (§15.212).
        let foreign = self.foreign_enums();
        for (module, types) in &foreign {
            o.push_str(&format!("require_once __DIR__ . {};\n", php_str(&format!("/{module}.php"))));
            for (_, ascii) in types {
                o.push_str(&format!("use {}\\{};\n", php_name(&pascal(module)), php_name(ascii)));
            }
        }
        if !foreign.is_empty() {
            o.push('\n');
        }

        // Enums. A backed enum carries the source name, which is also the wire value
        // (§10.2), so the runner converts in one call and never keeps a table of its own.
        for (jp, ascii) in self.own_enums() {
            o.push_str(&self.php_enum(&jp, &ascii));
        }
        o.push_str(&self.php_enum_helpers());
        // Which values need which before them (§15.222).
        o.push_str(&self.php_prereqs());

        // The machine this function is one step of (§15.148).
        if let Some((en, init, fins)) = self.machine_consts() {
            let cls = php_name(&self.enum_names.get(&en).cloned().unwrap_or_default());
            // The values carried along with the state, and where a case starts (§15.217).
            let mut more = String::new();
            for mc in self.more_carried() {
                let v = match mc.kind() {
                    super::MoreKind::Enum => self.php_value(&mc.word(), &mc.ty),
                    super::MoreKind::Bool => mc.bool_word().to_string(),
                    super::MoreKind::Number | super::MoreKind::Branded => mc.int.to_string(),
                };
                more.push_str(&format!("/** {} */\nconst {} = {v};\n\n", tr!("持ち越す {} の始まりの値。", "Where the carried {} starts.", mc.input), mc.upper()));
            }
            o.push_str(&format!(
                // `final` is a keyword in PHP whatever its case, so the set cannot be `FINAL`.
                "/** {} */\nconst INITIAL = {};\n\n/** {} */\nconst FINAL_STATES = [{}];\n\n{more}\
                 /** {} */\nfunction is_final({cls} $state): bool\n{{\n    return in_array($state, FINAL_STATES, true);\n}}\n\n",
                tr!("案件が始まる状態。", "The state a case starts in."),
                self.php_value(&init, &Ty::Enum(en.clone())),
                tr!("案件が終わる状態。", "The states a case ends in."),
                fins.iter().map(|v| self.php_value(v, &Ty::Enum(en.clone()))).collect::<Vec<_>>().join(", "),
                tr!("この状態で案件が終わっているか。", "Whether a case in this state has ended.")
            ));
        }

        o.push_str(&format!(
            "/** {} */\nfinal class RuleInputError extends \\InvalidArgumentException\n{{\n    \
             public function __construct(\n        public readonly string $what,\n        \
             public readonly ?int $value = null,\n    ) {{\n        \
             parent::__construct($value === null ? $what : \"{{$what}}: {{$value}}\");\n    }}\n}}\n\n",
            tr!("宣言した範囲の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller.")
        ));
        o.push_str(&format!(
            "/** {} */\nfinal class RuleContradictionError extends \\RuntimeException\n{{\n    \
             public function __construct(public readonly string $what)\n    {{\n        \
             parent::__construct($what);\n    }}\n}}\n\n",
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller.")
        ));
        o.push_str(&format!(
            "/** {} */\nfinal class Fired\n{{\n    public function __construct(\n        \
             public readonly string $table,\n        public readonly int $row,\n        \
             public readonly string $label = '',\n    ) {{\n    }}\n}}\n\n",
            fired_doc()
        ));

        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.php_value(&m.text, &self.group_ty(&g.name.text))).collect();
            o.push_str(&format!(
                "/** {} */\nconst {} = [{}];\n\n",
                g.name.text,
                self.php_group(&g.name.text),
                ms.join(", ")
            ));
        }

        o.push_str(round_php().trim_start_matches('\n'));
        o.push_str(&super::arith::php_helpers(&super::arith::Uses::of(self.f)));
        o.push('\n');
        o.push_str(&self.php_fn());
        o.push('\n');
        o.push_str(&self.php_record());
        if self.projects() {
            o.push('\n');
            o.push_str(&self.php_from());
        }
        // The function that counts the answers (§15.225).
        if self.f.has_random() {
            o.push('\n');
            o.push_str(self.php_chances().trim_end());
            o.push('\n');
        }
        o
    }

    /// The rule's items as PHP, at the base indentation (§15.56).
    fn php_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for (i, it) in self.f.items.iter().enumerate() {
            if !self.in_phase(i, it, phase) {
                continue;
            }
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!(
                        "    {} = {};  // {}\n",
                        php_var(&self.ident(&d.name.text)),
                        php_expr(unparen(&e.text)),
                        tr!("導出", "derived value")
                    ));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, local);
                    o.push_str(&format!(
                        "    {} = {};  // {}\n",
                        php_var(&self.ident(&d.name.text)),
                        php_expr(unparen(&e.text)),
                        tr!("定義", "definition")
                    ));
                }
                Item::Agg(_) | Item::Fold(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.php_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// Every walk of a rule that walks sequences, in PHP (§15.56, §15.58, §15.219). The shape is
    /// the one `py_walks` writes.
    fn php_walks(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            if let Some(cap) = self.count_cap_of(s) {
                let seq = php_var(&pub_name(&el.name));
                o.push_str(&format!("    if (count({seq}) > {cap}) {{\n"));
                o.push_str(&format!(
                    "        throw new RuleInputError({}, count({seq}));\n    }}\n",
                    php_str(&crate::door::too_many(&el.name.text, cap))
                ));
            }
        }
        for step in self.walk_steps() {
            match step {
                // What a walk does not depend on is worked out once, before it (§15.208).
                WalkStep::Once(ix) => o.push_str(&self.php_items(outer, trace, Phase::Only(&ix))),
                WalkStep::Walk { seq, body, sums, .. } => o.push_str(&self.php_walk(seq, &body, &sums, outer, trace)),
            }
        }
        o
    }

    /// One walk over sequence `s`, in PHP (§15.219).
    fn php_walk(&self, s: usize, body_items: &[usize], sums: &[usize], outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = &self.f.elements[s];
        let seq = php_var(&pub_name(&el.name));
        let fields: BTreeMap<String, String> = el.fields.iter().map(|fd| (fd.name.text.clone(), php_prop(&pub_name(&fd.name)))).collect();
        let e = php_var(&self.elem_var(s));
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}->{f}"),
                None => outer(n),
            }
        };
        let mut o = String::new();
        let mut body = self.php_element_guards(s, &local);
        body.push_str(&self.php_items(&local, trace, Phase::Only(body_items)));
        let mut after = String::new();
        for &i in sums {
            match &self.f.items[i] {
                crate::ast::Item::Agg(d) if d.kind.extreme() => {
                    let (init, step) = self.php_extreme(d, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                }
                crate::ast::Item::Agg(d) => {
                    o.push_str(&format!("    {} = 0;  // {}\n", php_var(&self.ident(&d.name.text)), self.agg_word(d)));
                    let v = local(&d.column.text);
                    if d.kind == crate::ast::AggKind::Sum {
                        body.push_str(&format!("    {} += {v};  // {}\n", php_var(&self.ident(&d.name.text)), d.name.text));
                    } else {
                        let test = self.php_test(&v, &d.column.text, &d.value);
                        body.push_str(&format!(
                            "    if ({test}) {{  // {}\n        {} += 1;\n    }}\n",
                            d.name.text,
                            php_var(&self.ident(&d.name.text))
                        ));
                    }
                    if let Some((_, cap)) = self.sum_caps().into_iter().find(|(x, _)| x.name.text == d.name.text) {
                        let n = php_var(&self.ident(&d.name.text));
                        after.push_str(&format!(
                            "    if ({n} > {cap}) {{\n        throw new RuleInputError({}, {n});\n    }}\n",
                            php_str(&crate::door::out_of_range(&d.name.text)),
                        ));
                    }
                }
                crate::ast::Item::Fold(fold) => {
                    let (init, step, fin) = self.php_fold(i, fold, &seq, &local);
                    o.push_str(&init);
                    body.push_str(&step);
                    after.push_str(&fin);
                }
                _ => {}
            }
        }
        o.push_str(&format!("    foreach ({seq} as {e}) {{\n"));
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");
        o.push_str(&after);
        o
    }

    /// A test on a column of one element, in PHP: `count`'s, and the `where` of `max` and `min`.
    fn php_test(&self, v: &str, col: &str, value: &Option<crate::ast::Name>) -> String {
        let ty = self.ty_of(col);
        match (&ty, value) {
            (Ty::Bool, Some(w)) if w.text == crate::kw::FALSE => format!("!{v}"),
            (Ty::Bool, _) => v.to_string(),
            (_, Some(w)) => format!("{v} === {}", self.php_value(&w.text, &ty)),
            (_, None) => v.to_string(),
        }
    }

    /// A `max` or a `min`, in PHP (§15.219).
    fn php_extreme(&self, d: &crate::ast::AggDecl, local: &dyn Fn(&str) -> String) -> (String, String) {
        let n = php_var(&self.ident(&d.name.text));
        let set = php_var(&self.temp(&format!("{}_set", self.ident(&d.name.text))));
        let v = local(&d.column.text);
        let ty = self.ty_of(&d.column.text);
        let op = if d.kind == crate::ast::AggKind::Max { ">" } else { "<" };
        let (start, better) = match self.extreme_enum(d) {
            Some(en) => {
                let t = self.enum_type(&en);
                let w = self.extreme_empty_word(d).unwrap_or_default();
                (self.php_value(&w, &ty), format!("_rank_{t}({v}) {op} _rank_{t}({n})"))
            }
            None => (self.extreme_empty_int(d).unwrap_or_else(|| "0".into()), format!("{v} {op} {n}")),
        };
        let init = format!("    {n} = {start};  // {}\n    {set} = false;\n", self.agg_word(d));
        let mut step = String::new();
        let pad = match &d.filter {
            Some((w, want)) => {
                step.push_str(&format!("    if ({}) {{  // {}\n", self.php_test(&local(&w.text), &w.text, want), d.name.text));
                "    "
            }
            None => "",
        };
        step.push_str(&format!("{pad}    if (!{set} || {better}) {{\n{pad}        {n} = {v};\n{pad}        {set} = true;\n{pad}    }}\n"));
        if !pad.is_empty() {
            step.push_str("    }\n");
        }
        (init, step)
    }

    /// A fold, in PHP (§15.56).
    fn php_fold(&self, i: usize, fold: &crate::ast::FoldDecl, seq: &str, local: &dyn Fn(&str) -> String) -> (String, String, String) {
        let names = self.fold_names(i);
        let (answer, stopped, done) = (php_var(&names.answer), php_var(&names.stopped), php_var(&names.done));
        let (taken, kept, best) = (php_var(&names.taken), php_var(&names.kept), php_var(&names.best));
        let (held, key) = (php_var(&names.held), php_var(&names.key));
        let alone = self.fold_alone(i);
        let with_held = |n: &str| -> String { if n == crate::kw::HELD { held.clone() } else { local(n) } };
        let (out_name, answer_ty) = self.fold_target(fold);
        // An answer is read in the type the fold answers with: a value of its enum is that
        // enum's member (§15.208).
        let val = |x: &crate::ast::Expr, l: &dyn Fn(&str) -> String| -> String {
            match x {
                crate::ast::Expr::Lit(Lit::Word(w), _) if w != crate::kw::TRUE && w != crate::kw::FALSE => self.php_value(w, &answer_ty),
                _ => php_expr(unparen(&self.expr(x, l).text)),
            }
        };
        let text = |x: &Option<crate::ast::Expr>| -> String { x.as_ref().map(|x| val(x, &with_held)).unwrap_or_else(|| "0".to_string()) };

        let mut init = String::new();
        init.push_str(&format!(
            "    {answer} = {};  // {}\n",
            text(&fold.empty),
            tr!("要素ゼロ件の答え", "the answer for no elements")
        ));
        init.push_str(&format!("    {stopped} = false;\n"));
        if !alone {
            init.push_str(&format!("    {done} = false;\n"));
        }
        init.push_str(&format!("    {taken} = null;\n    {kept} = null;\n    {best} = null;\n"));
        let weighted = fold.weighted();
        let draw = fold.draw.as_ref().map(|d| local(&d.text)).unwrap_or_default();
        let (total, weight, cuts) = (php_var(&names.total), php_var(&names.weight), php_var(&names.cuts));
        if weighted {
            init.push_str(&format!("    {total} = 0;  // {}\n    {cuts} = [];\n", tr!("重みの合計", "the running total of the weights")));
        }

        let mut arms = String::new();
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let is = self.php_value(&name.text, &self.ty_of(&fold.verdict));
            arms.push_str(&format!("    if ({v} === {is}) {{  // {}\n", name.text));
            match arm {
                Arm::Next => arms.push_str("        // next\n"),
                Arm::Stop(None) if alone => arms.push_str("        break;\n"),
                Arm::Stop(None) => arms.push_str(&format!("        {done} = true;\n")),
                Arm::Stop(Some(x)) => {
                    arms.push_str(&format!("        {answer} = {};\n", val(x, local)));
                    if alone {
                        arms.push_str(&format!("        {stopped} = true;\n        break;\n"));
                    } else {
                        arms.push_str(&format!("        {stopped} = true;\n        {done} = true;\n"));
                    }
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        arms.push_str(&format!("        if ({taken} !== null) {{\n"));
                        arms.push_str(&format!(
                            "            throw new RuleContradictionError({});\n        }}\n",
                            php_str(&tr!(
                                "畳み込み {}: take_unique に二件当たりました",
                                "fold {}: two elements matched a take_unique",
                                fold.verdict
                            ))
                        ));
                        arms.push_str(&format!("        {taken} = {};\n", val(expr, local)));
                    } else {
                        arms.push_str(&format!("        if ({taken} === null) {{\n"));
                        arms.push_str(&format!("            {taken} = {};\n        }}\n", val(expr, local)));
                    }
                }
                Arm::KeepMax { expr, key: k } => {
                    arms.push_str(&format!("        {key} = {};\n", php_expr(unparen(&self.expr(k, local).text))));
                    arms.push_str(&format!("        if ({best} === null || {key} > {best}) {{\n"));
                    arms.push_str(&format!("            {best} = {key};\n"));
                    arms.push_str(&format!("            {kept} = {};\n        }}\n", val(expr, local)));
                }
                // The weight goes on the running total, and the element is taken where the drawn
                // value is below the total for the first time (§15.226).
                Arm::TakeWeighted { expr, weight: wx } => {
                    arms.push_str(&format!("        {weight} = {};\n", php_expr(unparen(&self.expr(wx, local).text))));
                    arms.push_str(&format!("        {total} += {weight};\n        {cuts}[] = {total};\n"));
                    arms.push_str(&format!("        if ({taken} === null && {draw} < {total}) {{\n"));
                    arms.push_str(&format!("            {taken} = {};\n        }}\n", val(expr, local)));
                }
            }
            arms.push_str("    }\n");
        }
        let step = if alone { arms } else { format!("    if (!{done}) {{  // {}\n{}    }}\n", fold.verdict, Self::indent_block(&arms, "    ")) };

        // The drawn value of a pick by weight is below the total of the weights, unless they add up
        // to 0 (§15.226).
        let mut fin = String::new();
        if weighted {
            let what = fold.draw.as_ref().map(|d| crate::door::draw_past(&d.text)).unwrap_or_default();
            fin.push_str(&format!("    if ({total} > 0 && {draw} >= {total}) {{\n        throw new RuleInputError({}, {draw});\n    }}\n", php_str(&what)));
        }
        fin.push_str(&format!("    if (count({seq}) > 0 && !{stopped}) {{\n"));
        fin.push_str(&format!("        {held} = {taken} ?? {kept} ?? {};\n", text(&fold.empty)));
        fin.push_str(&format!("        {answer} = {};\n    }}\n", text(&fold.exhausted)));
        fin.push_str(&format!("    {} = {answer};\n", php_var(&self.ident(&out_name))));
        (init, step, fin)
    }

    /// The range guard of one element's fields, inside the walk. The kind of the value is
    /// the declared type's business here, so only the range is asked about.
    fn php_element_guards(&self, s: usize, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields(s) {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = super::Guarded::new(&mut o, optional.then(|| (format!("    if ({v} !== null) {{\n"), "    }\n".to_string())), "    ");
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "    if ({v} < {} || {v} > {}) {{\n        throw new RuleInputError({}, {v});\n    }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        php_str(&crate::door::out_of_range(&i.name.text)),
                    ));
                }
            }
        }
        o
    }

    fn php_fn(&self) -> String {
        let fname = self.php_fname();
        let outs = &self.f.outputs;
        let mut o = String::new();

        // One element is a row of inputs, so it gets the same shape as `Output`.
        for (s, el) in self.f.elements.iter().enumerate() {
            let fs: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    format!(
                        "        public readonly {} {},\n",
                        self.php_ty(&self.ty_of(&fd.name.text)),
                        php_var(&pub_name(&fd.name))
                    )
                })
                .collect();
            o.push_str(&format!(
                "/** {} */\nfinal class {}\n{{\n    public function __construct(\n{}    ) {{\n    }}\n}}\n\n",
                tr!("{} の一件", "one of {}", el.name.text),
                self.element_type(s),
                fs.join("")
            ));
        }

        // Multiple outputs come back as an object; one output is the value itself (§8.5).
        if outs.len() > 1 {
            let fs: Vec<String> = outs
                .iter()
                .map(|od| {
                    format!(
                        "        public readonly {} {},\n",
                        self.php_ty(&self.ty_of(&od.name.text)),
                        php_var(&pub_name(&od.name))
                    )
                })
                .collect();
            o.push_str(&format!(
                "final class Output\n{{\n    public function __construct(\n{}    ) {{\n    }}\n}}\n\n",
                fs.join("")
            ));
        }

        let traced = format!("{fname}_traced");
        let ret = if outs.len() == 1 {
            self.php_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{} {}", self.php_ty(&self.ty_of(&i.name.text)), php_var(&pub_name(&i.name))))
            .collect();
        for el in &self.f.elements {
            params.push(format!("array {}", php_var(&pub_name(&el.name))));
        }
        let args: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| php_var(&pub_name(&i.name)))
            .chain(self.f.elements.iter().map(|el| php_var(&pub_name(&el.name))))
            .collect();

        // The signature, with each parameter's declared type in the doc comment: this is
        // where the unit lives, since PHP cannot hold it.
        o.push_str(&format!("/**\n * {}\n", plain_doc(&self.f.name.text, &self.f.version, &traced)));
        for i in &self.f.inputs {
            o.push_str(&format!(
                " *   {} : {}\n",
                pub_name(&i.name),
                self.php_doc_ty(&self.ty_of(&i.name.text)).replace("int  // ", "")
            ));
        }
        for (s, el) in self.f.elements.iter().enumerate() {
            o.push_str(&format!(" *   {} : {}[]\n", pub_name(&el.name), self.element_type(s)));
        }
        for od in outs {
            o.push_str(&format!(
                " * -> {} : {}\n",
                pub_name(&od.name),
                self.php_doc_ty(&self.ty_of(&od.name.text)).replace("int  // ", "")
            ));
        }
        o.push_str(" */\n");
        o.push_str(&format!(
            "function {fname}({}): {ret}\n{{\n    return {traced}({})[0];\n}}\n\n",
            params.join(", "),
            args.join(", ")
        ));

        o.push_str(&format!(
            "/**\n * {}\n *\n * @return array{{0: {ret}, 1: list<Fired>}}\n */\n",
            traced_doc(&self.f.name.text, &self.f.version)
        ));
        o.push_str(&format!("function {traced}({}): array\n{{\n", params.join(", ")));
        // A rule that picks by weight runs its walk in a function of its own, which gives back the
        // running totals of the weights too (§15.225).
        let walked = self.weighted_fold().map(|(i, _)| (self.walked_fn("php"), php_var(&self.fold_names(i).cuts)));
        if let Some((w, _)) = &walked {
            o.push_str(&format!("    $r = {w}({});\n    return [$r[0], $r[1]];\n}}\n\n", args.join(", ")));
            o.push_str(&format!(
                "/**\n * {}\n *\n * @return array{{0: {ret}, 1: list<Fired>, 2: list<int>}}\n */\nfunction {w}({}): array\n{{\n",
                walked_doc(),
                params.join(", ")
            ));
        }

        // Entry guards (§8.5). The declared types already refuse a value of the wrong kind —
        // a float where an int was asked for, a string outside the enum — so what is left to
        // ask is the range (§15.43).
        let local = |n: &str| -> String { php_var(&self.ident(n)) };
        for i in &self.f.inputs {
            let v = php_var(&pub_name(&i.name));
            let ty = self.ty_of(&i.name.text);
            // An optional input is guarded as the type it wraps whenever it has a value (§15.201).
            let optional = matches!(ty, Ty::Opt(_));
            let ty = ty.present().clone();
            let mut o = super::Guarded::new(&mut o, optional.then(|| (format!("    if ({v} !== null) {{\n"), "    }\n".to_string())), "    ");
            // The values the range of an ordered enum lets in, first to last (§15.210).
            if let (Ty::Enum(n), Some((lo, hi))) = (&ty, self.c.enum_ranges.get(&i.name.text).and_then(|vs| Some((vs.first()?, vs.last()?)))) {
                let r = self.php_fn_of(n, "rank");
                o.push_str(&format!(
                    "    if ({r}({v}) < {r}({}) || {r}({v}) > {r}({})) {{\n        throw new RuleInputError({});\n    }}\n",
                    self.php_value(lo, &ty),
                    self.php_value(hi, &ty),
                    php_str(&crate::door::out_of_range(&i.name.text)),
                ));
            }
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "    if ({v} < {} || {v} > {}) {{\n        throw new RuleInputError({}, {v});\n    }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        php_str(&crate::door::out_of_range(&i.name.text)),
                    ));
                }
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Php, "    ", |m| {
            format!("        throw new RuleInputError({});\n", php_str(m))
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("    {} = [];\n", php_var(&trace)));

        if self.f.walks() {
            o.push_str(&self.php_walks(&local, &trace));
        } else {
            o.push_str(&self.php_items(&local, &trace, Phase::All));
        }

        // Every output: its source, brought to the wire scale, then its rounding once. An
        // optional output takes those steps only when its column has a value, and is `null` when
        // it has none (§15.249).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let opt = self.opt_out(oi);
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let res = self.onto_wire(res, os, od);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os && !opt {
                        let raw = php_var(&self.temp(&raw_base(oi)));
                        o.push_str(&format!(
                            "    {raw} = {};  // {}\n",
                            php_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("intdiv(_round_{}({raw}, {}), {})", mode_fn(m), grid_i, res.scale / os)
                    } else if res.scale != os {
                        format!("intdiv(_round_{}({}, {}), {})", mode_fn(m), php_expr(unparen(&res.text)), grid_i, res.scale / os)
                    } else {
                        format!("_round_{}({}, {})", mode_fn(m), php_expr(&res.text), grid_i)
                    }
                }
                None => php_expr(&res.text),
            };
            let v = local(out_name);
            finals.push(if opt && body != v { format!("{v} === null ? null : {body}") } else { body });
        }
        if outs.len() == 1 {
            let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
            o.push_str(&format!("    return [{}, {}{cuts}];\n", finals[0], php_var(&trace)));
        } else {
            let cuts = walked.as_ref().map(|(_, c)| format!(", {c}")).unwrap_or_default();
            o.push_str(&format!("    return [new Output({}), {}{cuts}];\n", finals.join(", "), php_var(&trace)));
        }
        o.push_str("}\n");
        o
    }

    fn php_table(&self, t: &Table, local: &dyn Fn(&str) -> String, trace: &str) -> String {
        let mut o = format!("    // {}\n", self.table_head(t));
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.php_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let kw = if ri == 0 { "if" } else { "} elseif" };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            o.push_str(&format!("    {kw} ({cond}) {{  // {}\n", self.row_head(t, ri, &cells.join(" | "))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.php_value(w, &self.ty_of(&oc.name.text)),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => super::str_lit(x),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.is_value(w) {
                            self.php_value(w, &self.ty_of(&oc.name.text))
                        } else {
                            let v = local(w);
                            match php_expr(&self.rescaled(w, &oc.name.text, v.clone())) {
                                // A value that may be absent is scaled only when it is there.
                                r if r != v && matches!(self.ty_of(w), Ty::Opt(_)) => format!("{v} === null ? null : {r}"),
                                r => r,
                            }
                        }
                    }
                    Some(OutCell::Nothing) => "null".into(),
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v};\n", php_var(&self.ident(&oc.name.text))));
            }
            let (tn, rn, lbl) = self.fired_of(t, ri);
            if lbl.is_empty() {
                o.push_str(&format!("        {}[] = new Fired({}, {rn});\n", php_var(trace), php_str(&tn)));
            } else {
                o.push_str(&format!(
                    "        {}[] = new Fired({}, {rn}, {});\n",
                    php_var(trace),
                    php_str(&tn),
                    php_str(&lbl)
                ));
            }
        }
        o.push_str(&format!(
            "    }} else {{\n        throw new RuleContradictionError({});\n    }}\n",
            php_str(&tr!(
                "到達不能: 完全性は rulec が静的に検査済み",
                "unreachable: completeness was statically checked by rulec"
            ))
        ));
        o.push_str(&self.guards(t, local, Lang::Php, "    ", |name, i, j| {
            format!(
                "        throw new RuleContradictionError({});\n",
                php_str(&tr!(
                    "表 {name}: {i} と {j} が同時に当てはまりました",
                    "table {name}: {i} and {j} matched at the same time"
                ))
            )
        }));
        o
    }

    fn php_wire(e: &str, w: &Wire) -> String {
        match w {
            Wire::Enum => format!("_json_str({e}->value)"),
            Wire::Bool => format!("({e} ? 'true' : 'false')"),
            Wire::Int | Wire::Brand => format!("(string) {e}"),
            Wire::Date => format!("_json_str(_civil({e}))"),
            Wire::Str => format!("_json_str({e})"),
            Wire::Opt(w) => format!("({e} === null ? 'null' : {})", Self::php_wire(e, w)),
        }
    }

    /// One call as a record (§15.35): the same canonical JSON every other target writes,
    /// assembled by hand rather than by `json_encode`, whose escaping of `/`, of non-ASCII
    /// and of a tab is its own and would part company with the other ten.
    pub fn php_record(&self) -> String {
        let fname = self.php_fname();
        let single = self.f.outputs.len() == 1;
        let (out, trace, tag) = (self.temp("out"), self.temp("trace"), self.temp("tag"));
        let (v_out, v_trace, v_tag) = (php_var(&out), php_var(&trace), php_var(&tag));
        let (ins, obs) = self.record_fields(
            |a, _| php_var(a),
            |_, a, _| if single { v_out.clone() } else { format!("{v_out}->{}", php_prop(a)) },
        );
        let ret = if single {
            self.php_ty(&self.ty_of(&self.f.outputs[0].name.text))
        } else {
            "Output".into()
        };
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{} {}", self.php_ty(&self.ty_of(&i.name.text)), php_var(&pub_name(&i.name))))
            .collect();
        for el in &self.f.elements {
            params.push(format!("array {}", php_var(&pub_name(&el.name))));
        }

        let mut o = String::new();
        o.push_str(
            "function _json_str(string $s): string\n{\n    $o = '\"';\n    $n = strlen($s);\n    \
             for ($i = 0; $i < $n; $i++) {\n        $c = $s[$i];\n        \
             if ($c === '\"' || $c === '\\\\') {\n            $o .= '\\\\' . $c;\n        \
             } elseif (ord($c) < 32) {\n            $o .= sprintf('\\\\u%04x', ord($c));\n        \
             } else {\n            $o .= $c;\n        }\n    }\n    return $o . '\"';\n}\n\n",
        );
        if civil_needed(self.f, self.c) {
            o.push_str(
                "function _civil(int $days): string\n{\n    $z = $days + 719468;\n    \
                 $era = intdiv($z >= 0 ? $z : $z - 146096, 146097);\n    \
                 $doe = $z - $era * 146097;\n    \
                 $yoe = intdiv($doe - intdiv($doe, 1460) + intdiv($doe, 36524) - intdiv($doe, 146096), 365);\n    \
                 $doy = $doe - (365 * $yoe + intdiv($yoe, 4) - intdiv($yoe, 100));\n    \
                 $mp = intdiv(5 * $doy + 2, 153);\n    \
                 $d = $doy - intdiv(153 * $mp + 2, 5) + 1;\n    \
                 $m = $mp < 10 ? $mp + 3 : $mp - 9;\n    \
                 $y = $yoe + $era * 400 + ($m <= 2 ? 1 : 0);\n    \
                 return sprintf('%04d-%02d-%02d', $y, $m, $d);\n}\n\n",
            );
        }
        o.push_str(&format!("/** {} */\n", record_doc()));
        o.push_str(&format!(
            "function {fname}_record({}, {ret} {v_out}, array {v_trace}, string {v_tag} = ''): string\n{{\n",
            params.join(", ")
        ));
        let (v_ins, v_obs, v_rows, v_head) =
            (php_var(&self.temp("ins")), php_var(&self.temp("obs")), php_var(&self.temp("rows")), php_var(&self.temp("head")));
        let field = |(jp, e, w): &(String, String, Wire)| {
            format!("        '\"{jp}\":' . {},\n", Self::php_wire(e, w))
        };
        // The sequences go in first (§15.56, §15.219).
        let mut seq_first = String::new();
        let mut seq_parts = String::new();
        for (s, el) in self.f.elements.iter().enumerate() {
            let e = php_var(&self.temp("elem"));
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "'\"{}\":' . {}",
                        fd.name.text,
                        Self::php_wire(&format!("{e}->{}", php_prop(&pub_name(&fd.name))), &wire_of(&ty))
                    )
                })
                .collect();
            let v_els = php_var(&if s == 0 { self.temp("els") } else { self.temp(&format!("els{}", s + 1)) });
            seq_first.push_str(&format!(
                "    {v_els} = [];\n    foreach ({} as {e}) {{\n        {v_els}[] = '{{' . {} . '}}';\n    }}\n",
                php_var(&pub_name(&el.name)),
                inner.join(" . ',' . ")
            ));
            seq_parts.push_str(&format!("        '\"{}\":[' . implode(',', {v_els}) . ']',\n", el.name.text));
        }
        if !seq_first.is_empty() {
            seq_first.push_str(&format!(
                "    {v_ins} = implode(',', [\n{seq_parts}{}    ]);\n",
                ins.iter().map(field).collect::<String>()
            ));
        }
        if seq_first.is_empty() {
            o.push_str(&format!(
                "    {v_ins} = implode(',', [\n{}    ]);\n",
                ins.iter().map(field).collect::<String>()
            ));
        } else {
            o.push_str(&seq_first);
        }
        o.push_str(&format!(
            "    {v_obs} = implode(',', [\n{}    ]);\n",
            obs.iter().map(field).collect::<String>()
        ));
        let f = php_var(&self.temp("f"));
        o.push_str(&format!("    {v_rows} = [];\n    foreach ({v_trace} as {f}) {{\n"));
        o.push_str(&format!(
            "        {v_rows}[] = '{{\"table\":' . _json_str({f}->table) . ',\"row\":' . {f}->row\n            \
             . ({f}->label === '' ? '' : ',\"label\":' . _json_str({f}->label)) . '}}';\n    }}\n"
        ));
        o.push_str(&format!(
            "    {v_head} = {v_tag} === '' ? '{{' : '{{\"tag\":' . _json_str({v_tag}) . ',';\n"
        ));
        o.push_str(&format!(
            "    return {v_head} . '\"in\":{{' . {v_ins} . '}},\"observed\":{{' . {v_obs}\n        \
             . '}},\"trace\":[' . implode(',', {v_rows}) . ']}}';\n}}\n"
        ));
        o
    }

    /// A PHP runner that reads JSONL on stdin and prints one record per line. `ext/json` is
    /// compiled into every 8.x build and cannot be disabled, so this needs no composer. An input
    /// is read the way every door reads it (§15.204), and one the door refuses gets a line saying
    /// so in the place of the record.
    pub fn php_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let fname = self.php_fname();
        let ns = self.php_ns();
        let d = php_var(&self.temp("d"));
        let read = |ty: &Ty, src: &str, name: &str| -> String {
            let optional = matches!(ty, Ty::Opt(_));
            let take = format!(
                "_take({src}, {}, {}{})",
                php_str(name),
                php_str(&crate::door::missing(name)),
                if optional { ", true" } else { "" }
            );
            let one = |v: &str| -> String {
                match ty.present() {
                    Ty::Enum(n) => format!(
                        "_enum({}::class, {v}, {})",
                        self.php_enum_fqn(n),
                        php_str(&crate::door::not_in_enum(name, n))
                    ),
                    Ty::Bool => format!("_bool({v}, {})", php_str(&crate::door::not_bool(name))),
                    Ty::Date => format!("_date({v}, {})", php_str(&crate::door::not_date(name))),
                    Ty::Str => format!("_str({v}, {})", php_str(&crate::door::not_string(name))),
                    _ => format!("_int({v}, {})", php_str(&crate::door::not_integer(name))),
                }
            };
            if optional {
                format!("_maybe({take}, fn($x) => {})", one("$x"))
            } else {
                one(&take)
            }
        };
        let mut args: Vec<String> = self.f.inputs.iter().map(|i| read(&self.ty_of(&i.name.text), &d, &i.name.text)).collect();
        // The sequence a walk reads, one element at a time (§15.56).
        let mut seq = String::new();
        for (k, el) in self.f.elements.iter().enumerate() {
            let e = php_var(&self.temp("e"));
            let v = php_var(&if k == 0 { self.temp("els") } else { self.temp(&format!("els{}", k + 1)) });
            let s = &el.name.text;
            let obj = format!("_obj({e}, {})", php_str(&crate::door::not_object(s)));
            let fields: Vec<String> = el.fields.iter().map(|fd| read(&self.ty_of(&fd.name.text), &obj, &fd.name.text)).collect();
            seq.push_str(&format!(
                "        {v} = [];\n        foreach (_seq(_take({d}, {}, {}), {}) as {e}) {{\n            {v}[] = new \\{ns}\\{}({});\n        }}\n",
                php_str(s),
                php_str(&crate::door::missing(s)),
                php_str(&crate::door::not_sequence(s)),
                self.element_type(k),
                fields.join(", ")
            ));
            args.push(v);
        }
        // `--chances` reads a line's inputs as the vectors are read (§15.225).
        let chances_mode = self.php_chances_mode(&d, &seq, &args);
        let a = php_var(&self.temp("args"));
        let (r, trace, line) = (php_var(&self.temp("r")), php_var(&self.temp("trace")), php_var(&self.temp("line")));
        // A machine's traces (§15.148): the carried input is the state this language answered
        // to the call before, handed over as the value it is.
        let (v, state) = (php_var(&self.temp("v")), php_var(&self.temp("state")));
        let (mut head, mut body, mut tail) = (String::new(), format!("{d} = json_decode({line}, true, 512, JSON_THROW_ON_ERROR)['in'];\n"), String::new());
        if let (Some((en, _, _)), Some((cin, _)), Some(out)) = (self.machine_consts(), self.carried(), self.carried_out_alias()) {
            // The state's enum where it is declared: the rule's namespace, or that of the `enums`
            // file it is read from (§15.212, §15.238).
            let cls = self.php_enum_fqn(&en);
            if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == cin) {
                args[k] = format!("(isset({v}['step']) ? {state} : {})", args[k]);
            }
            // The values carried along with the state (§15.217): a local each, from the
            // module's constant, read where each trace starts and handed on from the answer.
            let (mut more_head, mut more_meta, mut more_start, mut more_tail) = (String::new(), Vec::<String>::new(), String::new(), String::new());
            for mc in self.more_carried() {
                let local = php_var(&self.temp(&format!("carried_{}", mc.in_alias)));
                if let Some(k) = self.f.inputs.iter().position(|i| i.name.text == mc.input) {
                    args[k] = format!("(isset({v}['step']) ? {local} : {})", args[k]);
                }
                more_head.push_str(&format!("{local} = \\{ns}\\{};\n", mc.upper()));
                let shown = if mc.kind() == super::MoreKind::Enum { format!("\\{ns}\\{}->value", mc.upper()) } else { format!("\\{ns}\\{}", mc.upper()) };
                more_meta.push(format!("{} => {shown}", php_str(&mc.input)));
                more_start.push_str(&format!("        {local} = {};\n", read(&mc.ty, &format!("{v}['carried']"), &mc.input)));
                more_tail.push_str(&format!("        {local} = {r}->{};\n", mc.out_alias));
            }
            let carried_meta = if more_meta.is_empty() { String::new() } else { format!(", 'carried' => [{}]", more_meta.join(", ")) };
            head = format!("{state} = \\{ns}\\INITIAL;\n{more_head}");
            body = format!(
                "{v} = json_decode({line}, true, 512, JSON_THROW_ON_ERROR);\n    \
                 if (isset({v}['machine'])) {{\n        \
                     $fin = array_values(array_map(fn($s) => $s->value, array_filter({cls}::cases(), fn($s) => \\{ns}\\is_final($s))));\n        \
                     echo json_encode(['initial' => \\{ns}\\INITIAL->value, 'final' => $fin{carried_meta}], JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES), \"\\n\";\n        \
                     continue;\n    }}\n    \
                 {d} = {v}['in'];\n    \
                 if (({v}['step'] ?? null) === 'start') {{\n        {state} = {cls}::from({v}['state']);\n{more_start}    }}\n"
            );
            tail = format!("    if (isset({v}['step'])) {{\n        {state} = {};\n{more_tail}    }}\n", self.next_state_of(&r, &format!("->{out}")));
        }
        format!(
            "<?php\n\n\
             // Code generated by rulec {}. DO NOT EDIT.\n\n\
             declare(strict_types=1);\n\n\
             // A warning on stdout would land in the middle of the records, so everything the\n\
             // engine has to say goes to stderr whatever the machine's php.ini says.\n\
             ini_set('display_errors', 'stderr');\n\n\
             require_once __DIR__ . '/{alias}.php';\n\n\
             {}\
             {}\
             {head}\
             while (({line} = fgets(STDIN)) !== false) {{\n    \
             {line} = trim({line});\n    \
             if ({line} === '') {{\n        continue;\n    }}\n    \
             {body}    \
             try {{\n\
             {seq}        \
             {a} = [{}];\n        \
             [{r}, {trace}] = \\{ns}\\{fname}_traced(...{a});\n    \
             }} catch (\\{ns}\\RuleInputError $e) {{\n        \
             _refused('input', $e->what);\n        \
             continue;\n    \
             }} catch (\\{ns}\\RuleContradictionError $e) {{\n        \
             _refused('contradiction', $e->what);\n        \
             continue;\n    \
             }}\n    \
             {a}[] = {r};\n    \
             {a}[] = {trace};\n    \
             echo \\{ns}\\{fname}_record(...{a}), \"\\n\";\n\
             {tail}\
             }}\n",
            ritsu_emit::header::VERSION,
            PHP_WIRE.replace("@NS@", &ns),
            self.php_labels_mode() + &self.php_prereq_mode() + &chances_mode,
            args.join(", "),
        )
    }

    /// What the PHP runner answers with `--labels` (§15.211): for each line, an enum's name and
    /// a value's, the display name the file gives.
    fn php_labels_mode(&self) -> String {
        let enums = self.labeled_enums();
        if enums.is_empty() {
            return String::new();
        }
        let mut arms = String::new();
        for (en, _, _) in &enums {
            let f = self.php_fn_of(en, "label");
            let f = if f.starts_with('\\') { f } else { format!("\\{}\\{f}", self.php_ns()) };
            arms.push_str(&format!("            {} => {f}({}::from($v)),\n", php_str(en), self.php_enum_fqn(en)));
        }
        format!(
            "if (in_array('--labels', $argv, true)) {{\n    while (($l = fgets(STDIN)) !== false) {{\n        [$en, $v] = explode(\"\\t\", rtrim($l, \"\\n\"), 2);\n        echo match ($en) {{\n{arms}            default => '?',\n        }}, \"\\n\";\n    }}\n    exit(0);\n}}\n\n"
        )
    }

    /// The type as the inventory states it for PHP. There are no brands here, so a number is
    /// `int` and the unit travels in the entry's own `unit` field.
    pub(super) fn php_api_ty(&self, ty: &Ty) -> String {
        self.php_ty(ty)
    }
}

/// The five modes of §7.3 in PHP, checked against the same reference cases the other ten
/// are checked against. `rulec test` runs it beside the generated module.
pub fn round_tests_php(u: &super::Uses) -> String {
    let mut o = format!(
        "<?php\n\n\
         // Code generated by rulec {}. DO NOT EDIT.\n\
         // {}\n\n\
         declare(strict_types=1);\n\n\
         ini_set('display_errors', 'stderr');\n",
        ritsu_emit::header::VERSION,
        tr!(
            "丸めの五つのモード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five rounding modes, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(&round_php());
    o.push_str(&super::arith::php_helpers(u));
    o.push_str("\n$cases = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    ['{}', {x}, {g}, {want}],\n", mode_fn(m)));
    }
    o.push_str("];\n\n$bad = 0;\nforeach ($cases as [$mode, $x, $g, $want]) {\n");
    o.push_str("    $got = ('_round_' . $mode)($x, $g);\n    if ($got === $want) {\n        continue;\n    }\n");
    o.push_str("    echo \"NG {$mode}({$x}, {$g}) = {$got}, want {$want}\\n\";\n    $bad++;\n}\n");
    let (arith, n) = super::arith::php_tests(u);
    o.push_str(&arith);
    o.push_str("if ($bad > 0) {\n    exit(1);\n}\n");
    let count = if n == 0 { "count($cases)" } else { "(count($cases) + count($arith))" };
    o.push_str(&format!("echo {};\n", tr!("'ok ' . {count} . \" 件\\n\"", "'ok ' . {count} . \" cases\\n\"")));
    o
}

impl<'a> Gen<'a> {
    /// A value read out of the object `o` at the key `k`, `at` naming `o`, held to `kind` (PHP,
    /// §15.234).
    fn php_obj_at(&self, kind: &super::from::Kind, o: &str, k: &str, at: &str) -> String {
        use super::from::Kind;
        let (kq, aq) = (php_str(k), php_str(at));
        match kind {
            Kind::Int => format!("_int_at({o}, {kq}, {aq})"),
            Kind::IntPb => format!("_int_pb(_at({o}, {kq}, {aq}), {})", php_str(&format!("{at}.{k}"))),
            Kind::Bool => format!("_bool_at({o}, {kq}, {aq})"),
            Kind::Str => format!("_str_at({o}, {kq}, {aq})"),
            Kind::Date => format!("_date_at({o}, {kq}, {aq})"),
            Kind::Num => format!("_num_at({o}, {kq}, {aq})"),
            Kind::Enum(n) => format!(
                "_enum_of({}::class, _at({o}, {kq}, {aq}), {})",
                self.php_enum_fqn(n),
                php_str(&crate::door::not_in_enum(&format!("{at}.{k}"), n))
            ),
        }
    }

    /// A value already read, `at` naming it, held to `kind` (PHP).
    fn php_obj_of(&self, kind: &super::from::Kind, v: &str, at: &str) -> String {
        use super::from::Kind;
        let aq = php_str(at);
        match kind {
            Kind::Int => format!("_int_of({v}, {aq})"),
            Kind::IntPb => format!("_int_pb({v}, {aq})"),
            Kind::Bool => format!("_bool_of({v}, {aq})"),
            Kind::Str => format!("_str_of({v}, {aq})"),
            Kind::Date => format!("_date_of({v}, {aq})"),
            Kind::Num => format!("_num_of({v}, {aq})"),
            Kind::Enum(n) => format!("_enum_of({}::class, {v}, {})", self.php_enum_fqn(n), php_str(&crate::door::not_in_enum(at, n))),
        }
    }

    /// The test of an element a `where` makes, in PHP. A number with a fraction is compared with
    /// `==`, which compares an integer and a float by their values.
    fn php_obj_test(&self, p: &super::Proj, t: (&crate::ast::Name, &Cell)) -> String {
        use super::from::{Kind, elem_at, test_kind};
        let (field, cell) = t;
        let (kind, v) = match &p.proto {
            Some(pp) => {
                let (j, n, z) = pp.elem.clone().unwrap_or((field.text.clone(), field.text.clone(), crate::projection::Zero::Str));
                let kind = test_kind(cell, None, Some(&z));
                let zero = super::pb_zero(&z, "false", &php_str);
                let at = elem_at(p);
                (kind.clone(), self.php_obj_of(&kind, &format!("_pget($e, {}, {}, {zero}, {})", php_str(&j), php_str(&n), php_str(&at)), &format!("{at}.{}", field.text)))
            }
            None => {
                let kind = test_kind(cell, p.test_kind.as_ref(), None);
                (kind.clone(), self.php_obj_at(&kind, "$e", &field.text, &elem_at(p)))
            }
        };
        let (is, isnt) = if kind == Kind::Num { ("==", "!=") } else { ("===", "!==") };
        let syn = super::Syn { is, isnt, and: "&&", or: "||", tru: "true", not: None };
        super::elem_cond_with(cell, &v, &syn, &|l| self.read_lit(l, &kind, "php"))
    }

    /// The value of an element `sum`, `max` and `min` read, in PHP.
    fn php_obj_of_field(&self, p: &super::Proj, of: &crate::ast::Name) -> String {
        use super::from::{Kind, elem_at};
        let at = format!("{}.{}", elem_at(p), of.text);
        match p.proto.as_ref().and_then(|pp| pp.of.clone()) {
            Some((j, n, z)) => {
                let kind = if matches!(p.ty, Ty::Date) { Kind::Date } else { Kind::IntPb };
                let zero = super::pb_zero(&z, "false", &php_str);
                self.php_obj_of(&kind, &format!("_pget($e, {}, {}, {zero}, {})", php_str(&j), php_str(&n), php_str(&elem_at(p))), &at)
            }
            None => {
                let kind = if matches!(p.ty, Ty::Date) { Kind::Date } else { Kind::Int };
                self.php_obj_at(&kind, "$e", &of.text, &elem_at(p))
            }
        }
    }

    /// The collection a walk reads, as a list, in PHP.
    fn php_obj_collection(&self, p: &super::Proj, root: &str) -> String {
        match &p.proto {
            Some(pp) => {
                let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", php_str(j), php_str(n))).collect();
                format!("_seq_of(_proto({root}, [{}], [], [], {}), {})", steps.join(", "), php_str(&super::from::base_of(p)), php_str(&p.shown))
            }
            None => {
                let (o, at) = self.walk_to_last(p, root, &|o, k, at| format!("_at({o}, {}, {})", php_str(k), php_str(at)));
                format!("_seq_at({o}, {}, {})", php_str(p.path.last().map(String::as_str).unwrap_or("")), php_str(&at))
            }
        }
    }

    /// One projected value in PHP: an input's, or a field's of an element read from `$e`
    /// (§15.125, §15.234).
    fn php_obj_read(&self, p: &super::Proj) -> String {
        use super::from::kind_of;
        let root = if p.root == "e" { "$e".to_string() } else { php_var_pub(&p.root) };
        match p.kind {
            crate::ast::ProjKind::Field => {
                let Some(kind) = kind_of(&p.ty, p.proto.is_some()) else { return "null".into() };
                let opt = matches!(p.ty, Ty::Opt(_));
                match &p.proto {
                    Some(pp) => {
                        let lit = |z: &crate::projection::Zero| super::pb_zero(z, "false", &php_str);
                        let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", php_str(j), php_str(n))).collect();
                        let (absent, unset) = self.proto_defaults(p, pp, &lit, "null");
                        let walk = format!("_proto({root}, [{}], {absent}, {unset}, {})", steps.join(", "), php_str(&super::from::base_of(p)));
                        let read = self.php_obj_of(&kind, &walk, &p.shown);
                        if opt { format!("({walk} === null ? null : {read})") } else { read }
                    }
                    None if opt => {
                        let keys: Vec<String> = p.path.iter().map(|s| php_str(s)).collect();
                        let walk = format!("_opt({root}, [{}], {})", keys.join(", "), php_str(&super::from::base_of(p)));
                        format!("({walk} === null ? null : {})", self.php_obj_of(&kind, &walk, &p.shown))
                    }
                    None => {
                        let (o, at) = self.walk_to_last(p, &root, &|o, k, at| format!("_at({o}, {}, {})", php_str(k), php_str(at)));
                        self.php_obj_at(&kind, &o, p.path.last().map(String::as_str).unwrap_or(""), &at)
                    }
                }
            }
            _ => {
                let seq = self.php_obj_collection(p, &root);
                let test = p.kind.test().map(|t| self.php_obj_test(p, t));
                let t = || test.clone().unwrap_or_else(|| "true".into());
                match p.kind {
                    // Every element is tested, as in every other language.
                    crate::ast::ProjKind::Any(..) => format!("array_reduce({seq}, fn($a, $e) => ({}) || $a, false)", t()),
                    crate::ast::ProjKind::All(..) => format!("array_reduce({seq}, fn($a, $e) => ({}) && $a, true)", t()),
                    crate::ast::ProjKind::Count(None) => format!("count({seq})"),
                    crate::ast::ProjKind::Count(Some(_)) => format!("count(array_filter({seq}, fn($e) => {}))", t()),
                    _ => {
                        let Some(of) = p.kind.of().cloned() else { return "0".into() };
                        let each = self.php_obj_of_field(p, &of);
                        let vals = match &test {
                            Some(c) => format!("array_merge([], ...array_map(fn($e) => {c} ? [{each}] : [], {seq}))"),
                            None => format!("array_map(fn($e) => {each}, {seq})"),
                        };
                        match p.kind {
                            crate::ast::ProjKind::Sum(..) => format!("array_sum({vals})"),
                            _ => format!("{}({vals}, {})", Self::extreme_helper(p.kind), self.proj_empty(p)),
                        }
                    }
                }
            }
        }
    }

    /// One sequence built out of the caller's object in PHP (§15.223): an object of the element's
    /// class for every element of the collection, its fields in the order they are declared.
    fn php_obj_sequence(&self, q: &super::SeqProj) -> String {
        let root = php_var_pub(&q.root);
        let seq = match &q.proto {
            Some(pp) => {
                let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", php_str(j), php_str(n))).collect();
                format!("_seq_of(_proto({root}, [{}], [], [], {}), {})", steps.join(", "), php_str(&super::from::seq_base(q)), php_str(&q.shown))
            }
            None => {
                let mut o = root;
                let mut at: String = q.shown.split('.').next().unwrap_or_default().to_string();
                for s in &q.path[..q.path.len().saturating_sub(1)] {
                    o = format!("_at({o}, {}, {})", php_str(s), php_str(&at));
                    at = format!("{at}.{s}");
                }
                format!("_seq_at({o}, {}, {})", php_str(q.path.last().map(String::as_str).unwrap_or("")), php_str(&at))
            }
        };
        let fields: Vec<String> = q.fields.iter().map(|p| self.php_obj_read(p)).collect();
        format!("array_map(fn($e) => new {}({}), {seq})", self.element_type(q.s), fields.join(", "))
    }

    /// The helpers the PHP projection function calls, the ones `body` uses and what they call
    /// (§15.234). The error carries no value: PHP's `RuleInputError` holds an integer, and a value
    /// that is not one is what is being refused (§15.204).
    fn php_from_helpers(&self, body: &str) -> String {
        use super::from::{calls, joined, says};
        let s = says();
        let j = |p: &(String, String), at: &str| joined(p, at, ".", &php_str);
        let names = [
            "_int_at", "_bool_at", "_str_at", "_date_at", "_num_at", "_seq_at", "_int_pb", "_int_of", "_bool_of", "_str_of", "_date_of", "_num_of",
            "_seq_of", "_enum_of", "_opt", "_pget", "_at",
        ];
        let mut want: Vec<&str> = names.iter().copied().filter(|n| calls(body, n)).collect();
        for (fused, of) in [("_int_at", "_int_of"), ("_bool_at", "_bool_of"), ("_str_at", "_str_of"), ("_date_at", "_date_of"), ("_num_at", "_num_of"), ("_seq_at", "_seq_of"), ("_int_pb", "_int_of")] {
            if want.contains(&fused) {
                if !want.contains(&of) {
                    want.push(of);
                }
                if fused.ends_with("_at") && !want.contains(&"_at") {
                    want.push("_at");
                }
            }
        }
        let not_object = |v: &str| format!("!is_array({v}) || ({v} !== [] && array_is_list({v}))");
        let at_key = "$at . '.' . $k";
        let mut o = String::new();
        if want.contains(&"_at") {
            o.push_str(&format!(
                "function _at(mixed $o, string $k, string $at): mixed\n{{\n    if ({}) {{\n        throw new RuleInputError({});\n    }}\n    $v = $o[$k] ?? null;\n    if ($v === null) {{\n        throw new RuleInputError({});\n    }}\n    return $v;\n}}\n\n",
                not_object("$o"),
                j(&s.object, "$at"),
                j(&s.missing, at_key)
            ));
        }
        if want.contains(&"_opt") {
            o.push_str(&format!(
                "function _opt(mixed $o, array $keys, string $at): mixed\n{{\n    $v = $o;\n    $here = $at;\n    foreach ($keys as $k) {{\n        if ($v === null) {{\n            return null;\n        }}\n        if ({}) {{\n            throw new RuleInputError({});\n        }}\n        $here = $here . '.' . $k;\n        $v = $v[$k] ?? null;\n    }}\n    return $v;\n}}\n\n",
                not_object("$v"),
                j(&s.object, "$here")
            ));
        }
        if want.contains(&"_pget") {
            o.push_str(&format!(
                "function _pget(mixed $e, string $j, string $n, mixed $absent, string $at): mixed\n{{\n    if ({}) {{\n        throw new RuleInputError({});\n    }}\n    return $e[$j] ?? $e[$n] ?? $absent;\n}}\n\n",
                not_object("$e"),
                j(&s.object, "$at")
            ));
        }
        if want.contains(&"_seq_of") {
            o.push_str(&format!(
                "function _seq_of(mixed $v, string $at): array\n{{\n    if (!is_array($v) || !array_is_list($v)) {{\n        throw new RuleInputError({});\n    }}\n    return $v;\n}}\n\n",
                j(&s.sequence, "$at")
            ));
        }
        if want.contains(&"_int_of") {
            o.push_str(&format!(
                "function _int_of(mixed $v, string $at): int\n{{\n    if (is_float($v) && is_finite($v) && floor($v) == $v && $v >= -9.2233720368547758E18 && $v < 9.2233720368547758E18) {{\n        return (int) $v;\n    }}\n    if (!is_int($v)) {{\n        throw new RuleInputError({});\n    }}\n    return $v;\n}}\n\n",
                j(&s.integer, "$at")
            ));
        }
        if want.contains(&"_int_pb") {
            o.push_str(&format!(
                "function _int_pb(mixed $v, string $at): int\n{{\n    if (is_string($v)) {{\n        if (preg_match('/^-?[0-9]+$/D', $v) !== 1) {{\n            throw new RuleInputError({});\n        }}\n        return (int) $v;\n    }}\n    return _int_of($v, $at);\n}}\n\n",
                j(&s.integer, "$at")
            ));
        }
        for (name, test, ret, said) in [("_bool_of", "is_bool($v)", "bool", &s.boolean), ("_str_of", "is_string($v)", "string", &s.string), ("_num_of", "is_int($v) || is_float($v)", "int|float", &s.number)] {
            if want.contains(&name) {
                o.push_str(&format!(
                    "function {name}(mixed $v, string $at): {ret}\n{{\n    if (!({test})) {{\n        throw new RuleInputError({});\n    }}\n    return $v;\n}}\n\n",
                    j(said, "$at")
                ));
            }
        }
        if want.contains(&"_date_of") {
            o.push_str(&format!(
                "function _date_of(mixed $v, string $at): int\n{{\n    if (!is_string($v) || preg_match('/^[0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}$/D', $v) !== 1) {{\n        throw new RuleInputError({said});\n    }}\n    [$y, $m, $d] = array_map('intval', explode('-', $v));\n    if (!checkdate($m, $d, $y)) {{\n        throw new RuleInputError({said});\n    }}\n    return _days($v);\n}}\n\n",
                said = j(&s.date, "$at")
            ));
        }
        if want.contains(&"_enum_of") {
            o.push_str(
                "/** @param class-string<\\BackedEnum> $cls */\nfunction _enum_of(string $cls, mixed $v, string $what): \\BackedEnum\n{\n    $e = is_string($v) ? $cls::tryFrom($v) : null;\n    if ($e === null) {\n        throw new RuleInputError($what);\n    }\n    return $e;\n}\n\n",
            );
        }
        for (fused, of, ret) in [("_int_at", "_int_of", "int"), ("_bool_at", "_bool_of", "bool"), ("_str_at", "_str_of", "string"), ("_date_at", "_date_of", "int"), ("_num_at", "_num_of", "int|float"), ("_seq_at", "_seq_of", "array")] {
            if want.contains(&fused) {
                o.push_str(&format!("function {fused}(mixed $o, string $k, string $at): {ret}\n{{\n    return {of}(_at($o, $k, $at), {at_key});\n}}\n\n"));
            }
        }
        o
    }

    /// The parameters of the PHP projection function, with their types: the objects, the inputs
    /// no `from` reaches, and the sequences no `from` takes out of an object (§15.223).
    fn php_from_params(&self) -> Vec<String> {
        let mut params: Vec<String> = self.proj_params().iter().map(|r| format!("array {}", php_var_pub(r))).collect();
        params.extend(
            self.proj_rest()
                .iter()
                .map(|i| format!("{} {}", self.php_ty(&self.ty_of(&i.name.text)), php_var_pub(&super::pub_name(&i.name)))),
        );
        for s in self.proj_seq_rest() {
            params.push(format!("array {}", php_var_pub(&super::pub_name(&self.f.elements[s].name))));
        }
        params
    }

    /// The projection function in PHP, with the helpers it calls above it (§15.125, §15.234).
    pub(super) fn php_from(&self) -> String {
        use super::from::{calls, joined, says};
        if !self.projects() {
            return String::new();
        }
        let fname = self.php_fname();
        let mut body = format!("/** {} */\nfunction {fname}_from({})\n{{\n    return {fname}(\n", self.proj_doc(), self.php_from_params().join(", "));
        let projs = self.projections();
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            args.push(match projs.iter().find(|p| p.jp == i.name.text) {
                Some(p) => self.php_obj_read(p),
                None => php_var_pub(&super::pub_name(&i.name)),
            });
        }
        let seqs = self.seq_projections("e");
        for (s, el) in self.f.elements.iter().enumerate() {
            args.push(match seqs.iter().find(|q| q.s == s) {
                Some(q) => self.php_obj_sequence(q),
                None => php_var_pub(&super::pub_name(&el.name)),
            });
        }
        for a in &args {
            body.push_str(&format!("        {a},\n"));
        }
        body.push_str("    );\n}\n");
        let mut o = String::new();
        if calls(&body, "_proto") {
            // A place on the way that is not an object is refused, as `_at` refuses it; one that
            // is not there, or is `null`, is protojson's default (§15.234).
            o.push_str(&format!(
                "function _proto($o, array $path, $absent, $unset, string $at)\n{{\n    \
                 foreach ($path as $i => [$j, $p]) {{\n        \
                 if (!is_array($o) || ($o !== [] && array_is_list($o))) {{\n            \
                 throw new RuleInputError({});\n        \
                 }}\n        \
                 $v = $o[$j] ?? $o[$p] ?? null;\n        \
                 if ($v === null) {{\n            \
                 return $i === count($path) - 1 ? $absent : $unset;\n        \
                 }}\n        \
                 $o = $v;\n        \
                 $at = $at . '.' . $p;\n    \
                 }}\n    \
                 return $o;\n}}\n\n",
                joined(&says().object, "$at", ".", &php_str)
            ));
        }
        let helpers = self.php_from_helpers(&body);
        if calls(&helpers, "_days") {
            o.push_str(
                "function _days(string $s): int\n{\n    \
                 [$y, $m, $d] = array_map('intval', explode('-', $s));\n    \
                 $y -= $m <= 2 ? 1 : 0;\n    \
                 $era = intdiv($y >= 0 ? $y : $y - 399, 400);\n    \
                 $yoe = $y - $era * 400;\n    \
                 $doy = intdiv(153 * ($m + ($m > 2 ? -3 : 9)) + 2, 5) + $d - 1;\n    \
                 return $era * 146097 + $yoe * 365 + intdiv($yoe, 4) - intdiv($yoe, 100) + $doy - 719468;\n}\n\n",
            );
        }
        o.push_str(&helpers);
        // The largest and the smallest value of a field over a collection, or the value for none
        // (§15.223).
        for (name, f) in [("_largest", "max"), ("_smallest", "min")] {
            if calls(&body, name) {
                o.push_str(&format!("function {name}(array $vs, int $empty): int\n{{\n    return count($vs) === 0 ? $empty : {f}($vs);\n}}\n\n"));
            }
        }
        o.push_str(&body);
        o
    }
}

impl Gen<'_> {
    /// The projection function's signature, for the inventory (§15.125).
    pub(super) fn php_from_signature(&self) -> String {
        format!("function {}_from({})", self.php_fname(), self.php_from_params().join(", "))
    }
}

/// How the generated PHP runner reads the wire (§15.204): the readers of the Python runner's, in
/// PHP. The error carries no value here, because PHP's `RuleInputError` holds an integer and a
/// value that is not one is what is being refused.
const PHP_WIRE: &str = r#"/// A date as its day number from 1970-01-01, by the same civil-date arithmetic the tool uses.
function _ord(string $s): int
{
    [$y, $m, $d] = array_map('intval', explode('-', $s));
    $y2 = $m <= 2 ? $y - 1 : $y;
    $era = intdiv($y2 >= 0 ? $y2 : $y2 - 399, 400);
    $yoe = $y2 - $era * 400;
    $mp = ($m + 9) % 12;
    $doy = intdiv(153 * $mp + 2, 5) + $d - 1;
    $doe = $yoe * 365 + intdiv($yoe, 4) - intdiv($yoe, 100) + $doy;
    return $era * 146097 + $doe - 719468;
}

function _take(mixed $d, string $k, string $missing, bool $optional = false): mixed
{
    if (!is_array($d) || !array_key_exists($k, $d) || ($d[$k] === null && !$optional)) {
        throw new \@NS@\RuleInputError($missing);
    }
    return $d[$k];
}

function _int(mixed $v, string $what): int
{
    // A whole number written with a point or an exponent, 1000.0 or 1e3, is that integer: as
    // long as it is an int at all (§15.205). 2 ** 63 and past are not.
    if (is_float($v) && is_finite($v) && floor($v) == $v && $v >= -9.2233720368547758E18 && $v < 9.2233720368547758E18) {
        return (int) $v;
    }
    if (!is_int($v)) {
        throw new \@NS@\RuleInputError($what);
    }
    return $v;
}

function _bool(mixed $v, string $what): bool
{
    if (!is_bool($v)) {
        throw new \@NS@\RuleInputError($what);
    }
    return $v;
}

function _str(mixed $v, string $what): string
{
    if (!is_string($v)) {
        throw new \@NS@\RuleInputError($what);
    }
    return $v;
}

function _date(mixed $v, string $what): int
{
    if (!is_string($v) || preg_match('/^[0-9]{4}-[0-9]{2}-[0-9]{2}$/D', $v) !== 1) {
        throw new \@NS@\RuleInputError($what);
    }
    [$y, $m, $d] = array_map('intval', explode('-', $v));
    if (!checkdate($m, $d, $y)) {
        throw new \@NS@\RuleInputError($what);
    }
    return _ord($v);
}

/** @param class-string<\BackedEnum> $cls */
function _enum(string $cls, mixed $v, string $what): \BackedEnum
{
    $e = is_string($v) ? $cls::tryFrom($v) : null;
    if ($e === null) {
        throw new \@NS@\RuleInputError($what);
    }
    return $e;
}

/** @return list<mixed> */
function _seq(mixed $v, string $what): array
{
    if (!is_array($v) || !array_is_list($v)) {
        throw new \@NS@\RuleInputError($what);
    }
    return $v;
}

/** @return array<string, mixed> */
function _obj(mixed $v, string $what): array
{
    if (!is_array($v) || ($v !== [] && array_is_list($v))) {
        throw new \@NS@\RuleInputError($what);
    }
    return $v;
}

function _maybe(mixed $v, callable $read): mixed
{
    return $v === null ? null : $read($v);
}

function _refused(string $kind, string $what): void
{
    echo json_encode(['refused' => $kind, 'error' => $what], JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES), "\n";
}

"#;

// ── The function that counts the answers (§15.225) ──────────────────────────────

impl<'a> Gen<'a> {
    /// The `Chances` class and the function, in PHP.
    pub(super) fn php_chances(&self) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let (doc_fn, doc_ty) = self.chances_docs();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 { self.php_ty(&self.ty_of(&outs[0].name.text)) } else { "Output".into() };
        let fname = self.php_fname();
        let name = self.chances_name("php");
        let mut params: Vec<String> = self
            .f
            .inputs
            .iter()
            .filter(|i| i.random.is_none())
            .map(|i| format!("{} {}", self.php_ty(&self.ty_of(&i.name.text)), php_var(&pub_name(&i.name))))
            .collect();
        for el in &self.f.elements {
            params.push(format!("array {}", php_var(&pub_name(&el.name))));
        }
        let call = |draw: Option<(&str, &str)>| -> String {
            let mut args: Vec<String> = self
                .f
                .inputs
                .iter()
                .map(|i| match draw {
                    Some((d, v)) if i.name.text == d => v.to_string(),
                    _ => php_var(&pub_name(&i.name)),
                })
                .collect();
            for el in &self.f.elements {
                args.push(php_var(&pub_name(&el.name)));
            }
            args.join(", ")
        };
        let (total, ways, out, k, last, e) = (
            php_var(&self.temp("total")),
            php_var(&self.temp("ways")),
            php_var(&self.temp("out")),
            php_var(&self.temp("k")),
            php_var(&self.temp("last")),
            php_var(&self.temp("e")),
        );
        let add = |ind: &str, n: &str| -> String {
            format!(
                "{ind}{k} = 0;\n{ind}while ({k} < count({ways}) && {ways}[{k}][0] != {out}) {{\n{ind}    {k} += 1;\n{ind}}}\n{ind}if ({k} === count({ways})) {{\n{ind}    {ways}[] = [{out}, {n}];\n{ind}}} else {{\n{ind}    {ways}[{k}][1] += {n};\n{ind}}}\n"
            )
        };
        let mut o = format!(
            "/**\n * {doc_ty}\n */\nfinal class Chances\n{{\n    /**\n     * @param list<array{{0: {ret}, 1: int}}> $ways\n     */\n    public function __construct(\n        public readonly int $total,\n        public readonly array $ways,\n    ) {{\n    }}\n}}\n\n"
        );
        o.push_str(&format!("/**\n * {doc_fn}\n */\nfunction {name}({}): Chances\n{{\n", params.join(", ")));
        o.push_str(&format!("    {total} = 0;\n    {ways} = [];\n"));
        match self.weighted_draw_pub() {
            Some((draw, lo, hi)) => {
                let (cuts, low, cut, n) = (php_var(&self.temp("cuts")), php_var(&self.temp("low")), php_var(&self.temp("cut")), php_var(&self.temp("n")));
                o.push_str(&format!("    {cuts} = {}({})[2];\n", self.walked_fn("php"), call(Some((&draw, &lo.to_string())))));
                o.push_str(&format!(
                    "    if (count({cuts}) === 0 || {cuts}[count({cuts}) - 1] === 0) {{\n        return new Chances({all}, [[{fname}({}), {all}]]);\n    }}\n",
                    call(Some((&draw, &lo.to_string()))),
                    all = hi - lo + 1
                ));
                o.push_str(&format!("    {low} = {lo};\n    foreach ({cuts} as {cut}) {{\n        if ({cut} > {low} && {low} <= {hi}) {{\n"));
                o.push_str(&format!("            {n} = min({cut}, {}) - {low};\n", hi + 1));
                o.push_str(&format!("            {out} = {fname}({});\n", call(Some((&draw, &low)))));
                o.push_str(&add("            ", &n));
                o.push_str(&format!("            {total} += {n};\n        }}\n        if ({cut} > {low}) {{\n            {low} = {cut};\n        }}\n    }}\n    return new Chances({total}, {ways});\n}}\n\n"));
            }
            None => {
                o.push_str(&format!("    {last} = null;\n"));
                let mut ind = "    ".to_string();
                for (j, d) in self.drawn().iter().enumerate() {
                    let v = php_var(&d.alias);
                    match &d.from {
                        super::chances::From::Range(lo, hi) => {
                            let ctr = php_var(&self.temp(&format!("_i{j}")));
                            o.push_str(&format!("{ind}for ({ctr} = {lo}; {ctr} <= {hi}; {ctr}++) {{\n{ind}    {v} = {ctr};\n"));
                        }
                        super::chances::From::Values(vs) => {
                            let members: Vec<String> = vs.iter().map(|x| self.php_value(x, &d.ty)).collect();
                            o.push_str(&format!("{ind}foreach ([{}] as {v}) {{\n", members.join(", ")));
                        }
                        super::chances::From::Bool => o.push_str(&format!("{ind}foreach ([false, true] as {v}) {{\n")),
                    }
                    ind.push_str("    ");
                }
                o.push_str(&format!(
                    "{ind}try {{\n{ind}    {out} = {fname}({});\n{ind}}} catch (RuleInputError {e}) {{\n{ind}    {last} = {e};\n{ind}    continue;\n{ind}}}\n{ind}{total} += 1;\n",
                    call(None)
                ));
                o.push_str(&add(&ind, "1"));
                for _ in self.drawn() {
                    ind.truncate(ind.len() - 4);
                    o.push_str(&format!("{ind}}}\n"));
                }
                o.push_str(&format!("    if ({total} === 0 && {last} !== null) {{\n        throw {last};\n    }}\n    return new Chances({total}, {ways});\n}}\n\n"));
            }
        }
        o
    }

    /// What the PHP runner answers with `--chances`: `seq` reads the sequences of a line, `args`
    /// every input.
    pub(super) fn php_chances_mode(&self, d: &str, seq: &str, args: &[String]) -> String {
        if !self.f.has_random() {
            return String::new();
        }
        let ns = self.php_ns();
        let fname = self.php_fname();
        let name = self.chances_name("php");
        let n_in = self.f.inputs.len();
        let mut pass: Vec<String> = Vec::new();
        for (k, i) in self.f.inputs.iter().enumerate() {
            if i.random.is_none() {
                pass.push(format!("$ca[{k}]"));
            }
        }
        for s in 0..self.f.elements.len() {
            pass.push(format!("$ca[{}]", n_in + s));
        }
        r#"if (in_array('--chances', $argv, true)) {
    while (($cl = fgets(STDIN)) !== false) {
        $cl = trim($cl);
        if ($cl === '') {
            continue;
        }
        @D@ = json_decode($cl, true, 512, JSON_THROW_ON_ERROR)['in'];
@SEQ@        $ca = [@ARGS@];
        $c = \@NS@\@NAME@(@PASS@);
        echo '{"total":' . $c->total . '}', "\n";
        foreach ($c->ways as [$co, $cn]) {
            $rec = \@NS@\@FNAME@_record(...[...$ca, $co, []]);
            $a = strrpos($rec, ',"observed":') + 12;
            echo $cn, "\t", substr($rec, $a, strrpos($rec, ',"trace":') - $a), "\n";
        }
    }
    exit(0);
}

"#
        .replace("@D@", d)
        .replace("@SEQ@", seq)
        .replace("@ARGS@", &args.join(", "))
        .replace("@NS@", &ns)
        .replace("@NAME@", &name)
        .replace("@PASS@", &pass.join(", "))
        .replace("@FNAME@", &fname)
    }
}
