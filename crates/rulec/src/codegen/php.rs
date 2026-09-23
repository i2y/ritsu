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
    cell_src, civil_needed, fired_doc, mode_fn, out_src, pascal, plain_doc, pub_name, raw_base,
    record_doc, round_cases, traced_doc, unparen, wire_of, Expr2, Gen, Lang, Phase, Wire,
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
    fn php_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{}::{}", php_name(ty), php_name(&alias.to_uppercase())),
            None => php_str(v),
        }
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
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.php_value(w),
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
                        out.extend(ms.iter().map(|m| self.php_value(m)));
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
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} === {}", lit(l)),
            Cell::Set(ls) => format!("in_array({var}, {}, true)", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!in_array({var}, {}, true)", self.php_group(w))
            }
            Cell::Not(ls) => format!("!in_array({var}, {}, true)", members(ls)),
            Cell::Cmp(cs) => cs
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
                .join(" && "),
        })
    }

    /// The module: one file, no composer, no autoloader.
    pub fn php(&self) -> String {
        let mut o = String::from("<?php\n\n");
        o.push_str(&self.header("//"));
        o.push_str("\ndeclare(strict_types=1);\n\n");
        o.push_str(&format!("namespace {};\n\n", self.php_ns()));

        // Enums. A backed enum carries the source name, which is also the wire value
        // (§10.2), so the runner converts in one call and never keeps a table of its own.
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("/** {jp} */\nenum {}: string\n{{\n", php_name(ascii)));
            for v in vals {
                let name = php_name(
                    &self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone()),
                );
                o.push_str(&format!("    case {name} = {};\n", php_str(v)));
            }
            o.push_str("}\n\n");
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
            let ms: Vec<String> = g.members.iter().map(|m| self.php_value(&m.text)).collect();
            o.push_str(&format!(
                "/** {} */\nconst {} = [{}];\n\n",
                g.name.text,
                self.php_group(&g.name.text),
                ms.join(", ")
            ));
        }

        o.push_str(round_php().trim_start_matches('\n'));
        o.push('\n');
        o.push_str(&self.php_fn());
        o.push('\n');
        o.push_str(&self.php_record());
        if self.projects() {
            o.push('\n');
            o.push_str(&self.php_from());
        }
        o
    }

    /// The rule's items as PHP, at the base indentation (§15.56).
    fn php_items(&self, local: &dyn Fn(&str) -> String, trace: &str, phase: Phase) -> String {
        let mut o = String::new();
        for it in &self.f.items {
            if !self.in_phase(it, phase) {
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
                Item::Agg(_) => {}
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        o.push_str(&self.php_table(t, local, trace));
                    }
                }
            }
        }
        o
    }

    /// The walk, in PHP (§15.56).
    fn php_walk(&self, fold: &crate::ast::FoldDecl, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a fold has elements");
        let seq = php_var(&pub_name(&el.name));
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), php_name(&pub_name(&fd.name)))).collect();
        let (answer, stopped, taken, kept, best) = self.fold_locals();
        let (answer, stopped) = (php_var(&answer), php_var(&stopped));
        let (taken, kept, best) = (php_var(&taken), php_var(&kept), php_var(&best));
        let e = php_var(&self.temp("elem"));
        let held = self.temp("held");
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}->{f}"),
                None if n == crate::kw::HELD => php_var(&held),
                None => php_var(&self.ident(n)),
            }
        };
        let out_name = self.f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
        let text = |x: &Option<crate::ast::Expr>| -> String {
            x.as_ref().map(|x| php_expr(unparen(&self.expr(x, &local).text))).unwrap_or_else(|| "0".to_string())
        };

        let mut o = String::new();
        o.push_str(&format!(
            "    {answer} = {};  // {}\n",
            text(&fold.empty),
            tr!("要素ゼロ件の答え", "the answer for no elements")
        ));
        o.push_str(&format!("    {stopped} = false;\n"));
        o.push_str(&format!("    {taken} = null;\n    {kept} = null;\n    {best} = null;\n"));
        o.push_str(&format!("    foreach ({seq} as {e}) {{\n"));

        let mut body = self.php_element_guards(&local);
        body.push_str(&self.php_items(&local, trace, Phase::All));
        let v = local(&fold.verdict);
        for (name, arm, _) in &fold.arms {
            let val = self.php_value(&name.text);
            body.push_str(&format!("    if ({v} === {val}) {{  // {}\n", name.text));
            match arm {
                Arm::Next => body.push_str("        // next\n"),
                Arm::Stop(None) => body.push_str("        break;\n"),
                Arm::Stop(Some(x)) => {
                    body.push_str(&format!("        {answer} = {};\n", php_expr(unparen(&self.expr(x, &local).text))));
                    body.push_str(&format!("        {stopped} = true;\n        break;\n"));
                }
                Arm::Take { expr, unique } => {
                    if *unique {
                        body.push_str(&format!("        if ({taken} !== null) {{\n"));
                        body.push_str(&format!(
                            "            throw new RuleContradictionError({});\n        }}\n",
                            php_str(&tr!(
                                "畳み込み {}: take_unique に二件当たりました",
                                "fold {}: two elements matched a take_unique",
                                fold.verdict
                            ))
                        ));
                        body.push_str(&format!("        {taken} = {};\n", php_expr(unparen(&self.expr(expr, &local).text))));
                    } else {
                        body.push_str(&format!("        if ({taken} === null) {{\n"));
                        body.push_str(&format!(
                            "            {taken} = {};\n        }}\n",
                            php_expr(unparen(&self.expr(expr, &local).text))
                        ));
                    }
                }
                Arm::KeepMax { expr, key } => {
                    let k = php_var(&self.temp("key"));
                    body.push_str(&format!("        {k} = {};\n", php_expr(unparen(&self.expr(key, &local).text))));
                    body.push_str(&format!("        if ({best} === null || {k} > {best}) {{\n"));
                    body.push_str(&format!("            {best} = {k};\n"));
                    body.push_str(&format!(
                        "            {kept} = {};\n        }}\n",
                        php_expr(unparen(&self.expr(expr, &local).text))
                    ));
                }
            }
            body.push_str("    }\n");
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");

        o.push_str(&format!("    if (count({seq}) > 0 && !{stopped}) {{\n"));
        o.push_str(&format!("        {} = {taken} ?? {kept} ?? {};\n", php_var(&held), text(&fold.empty)));
        o.push_str(&format!("        {answer} = {};\n    }}\n", text(&fold.exhausted)));
        o.push_str(&format!("    {} = {answer};\n", php_var(&self.ident(&out_name))));
        o
    }

    /// The counting walk in PHP (§15.58).
    fn php_count_walk(&self, outer: &dyn Fn(&str) -> String, trace: &str) -> String {
        let el = self.f.elements.as_ref().expect("a count has elements");
        let seq = php_var(&pub_name(&el.name));
        let fields: BTreeMap<String, String> =
            el.fields.iter().map(|fd| (fd.name.text.clone(), php_name(&pub_name(&fd.name)))).collect();
        let e = php_var(&self.temp("elem"));
        let local = |n: &str| -> String {
            match fields.get(n) {
                Some(f) => format!("{e}->{f}"),
                None => php_var(&self.ident(n)),
            }
        };
        let mut o = String::new();
        if let Some(cap) = self.count_cap() {
            o.push_str(&format!("    if (count({seq}) > {cap}) {{\n"));
            o.push_str(&format!(
                "        throw new RuleInputError({}, count({seq}));\n    }}\n",
                php_str(&tr!(
                    "{} の要素が多すぎます（上限 {cap}）",
                    "{} has too many elements (at most {cap})",
                    el.name.text
                ))
            ));
        }
        for d in self.counts() {
            o.push_str(&format!(
                "    {} = 0;  // {}\n",
                php_var(&self.ident(&d.name.text)),
                self.agg_word(d)
            ));
        }
        o.push_str(&format!("    foreach ({seq} as {e}) {{\n"));
        let mut body = self.php_element_guards(&local);
        body.push_str(&self.php_items(&local, trace, Phase::Walk));
        for d in self.counts() {
            let v = local(&d.column.text);
            if d.kind == crate::ast::AggKind::Sum {
                body.push_str(&format!("    {} += {v};  // {}\n", php_var(&self.ident(&d.name.text)), d.name.text));
                continue;
            }
            let test = match self.count_member(d) {
                Some(w) => format!("{v} === {}", self.php_value(&w.text)),
                None if self.count_negated(d) => format!("!{v}"),
                None => v,
            };
            body.push_str(&format!(
                "    if ({test}) {{  // {}\n        {} += 1;\n    }}\n",
                d.name.text,
                php_var(&self.ident(&d.name.text))
            ));
        }
        o.push_str(&Self::indent_block(&body, "    "));
        o.push_str("    }\n");
        for (d, cap) in self.sum_caps() {
            let n = php_var(&self.ident(&d.name.text));
            o.push_str(&format!(
                "    if ({n} > {cap}) {{\n        throw new RuleInputError({}, {n});\n    }}\n",
                php_str(&tr!("{} が範囲の外です", "{} is out of range", d.name.text)),
            ));
        }
        o.push_str(&self.php_items(outer, trace, Phase::Main));
        o
    }

    /// The range guard of one element's fields, inside the walk. The kind of the value is
    /// the declared type's business here, so only the range is asked about.
    fn php_element_guards(&self, local: &dyn Fn(&str) -> String) -> String {
        let mut o = String::new();
        for i in self.element_fields() {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "    if ({v} < {} || {v} > {}) {{\n        throw new RuleInputError({}, {v});\n    }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        php_str(&tr!("{} が範囲の外です", "{} is out of range", i.name.text)),
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
        if let Some(el) = &self.f.elements {
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
                "/** {} */\nfinal class Element\n{{\n    public function __construct(\n{}    ) {{\n    }}\n}}\n\n",
                tr!("{} の一件", "one of {}", el.name.text),
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
        if let Some(el) = &self.f.elements {
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
        if let Some(el) = &self.f.elements {
            o.push_str(&format!(" *   {} : Element[]\n", pub_name(&el.name)));
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

        // Entry guards (§8.5). The declared types already refuse a value of the wrong kind —
        // a float where an int was asked for, a string outside the enum — so what is left to
        // ask is the range (§15.43).
        let local = |n: &str| -> String { php_var(&self.ident(n)) };
        for i in &self.f.inputs {
            let v = php_var(&pub_name(&i.name));
            let ty = self.ty_of(&i.name.text);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                    let sc = self.c.wire_scale(&i.name.text);
                    o.push_str(&format!(
                        "    if ({v} < {} || {v} > {}) {{\n        throw new RuleInputError({}, {v});\n    }}\n",
                        crate::types::wire_int(lo, sc),
                        crate::types::wire_int(hi, sc),
                        php_str(&tr!("{} が範囲の外です", "{} is out of range", i.name.text)),
                    ));
                }
            }
        }

        o.push_str(&self.constraint_guards(&local, Lang::Php, "    ", |m| {
            format!("        throw new RuleInputError({});\n", php_str(m))
        }));

        let trace = self.temp("trace");
        o.push_str(&format!("    {} = [];\n", php_var(&trace)));

        match &self.f.fold {
            Some(fold) => o.push_str(&self.php_walk(fold, &trace)),
            None if self.counts().is_empty() => o.push_str(&self.php_items(&local, &trace, Phase::All)),
            None => o.push_str(&self.php_count_walk(&local, &trace)),
        }

        // Every output: its source, brought to the wire scale, then its rounding once.
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = php_var(&self.temp(&raw_base(oi)));
                        o.push_str(&format!(
                            "    {raw} = {};  // {}\n",
                            php_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("intdiv(_round_{}({raw}, {}), {})", mode_fn(m), grid_i, res.scale / os)
                    } else {
                        format!("_round_{}({}, {})", mode_fn(m), php_expr(&res.text), grid_i)
                    }
                }
                None => php_expr(&res.text),
            });
        }
        if outs.len() == 1 {
            o.push_str(&format!("    return [{}, {}];\n", finals[0], php_var(&trace)));
        } else {
            o.push_str(&format!("    return [new Output({}), {}];\n", finals.join(", "), php_var(&trace)));
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
                        Lit::Word(w) => self.php_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        Lit::Str(x) => super::str_lit(x),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.php_value(w)
                        } else {
                            php_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
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
            |_, a, _| if single { v_out.clone() } else { format!("{v_out}->{}", php_name(a)) },
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
        if let Some(el) = &self.f.elements {
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
        // The sequence goes in first (§15.56).
        let mut seq_first = String::new();
        if let Some(el) = &self.f.elements {
            let e = php_var(&self.temp("elem"));
            let inner: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let ty = self.ty_of(&fd.name.text);
                    format!(
                        "'\"{}\":' . {}",
                        fd.name.text,
                        Self::php_wire(&format!("{e}->{}", php_name(&pub_name(&fd.name))), &wire_of(&ty))
                    )
                })
                .collect();
            let v_els = php_var(&self.temp("els"));
            seq_first = format!(
                "    {v_els} = [];\n    foreach ({} as {e}) {{\n        {v_els}[] = '{{' . {} . '}}';\n    }}\n",
                php_var(&pub_name(&el.name)),
                inner.join(" . ',' . ")
            );
            seq_first.push_str(&format!(
                "    {v_ins} = implode(',', [\n        '\"{}\":[' . implode(',', {v_els}) . ']',\n{}    ]);\n",
                el.name.text,
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
    /// compiled into every 8.x build and cannot be disabled, so this needs no composer.
    pub fn php_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let fname = self.php_fname();
        let ns = self.php_ns();
        let d = php_var(&self.temp("d"));
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let jp = &i.name.text;
            let k = php_str(jp);
            args.push(match &self.ty_of(&i.name.text) {
                Ty::Enum(n) => format!(
                    "\\{ns}\\{}::from({d}[{k}])",
                    php_name(&self.enum_names.get(n).cloned().unwrap_or_default())
                ),
                Ty::Str => format!("(string) {d}[{k}]"),
                Ty::Bool => format!("(bool) {d}[{k}]"),
                Ty::Date => format!("_ord({d}[{k}])"),
                // `null` on the wire is PHP's `null`, and the module's parameter is `?Kind`. It
                // used to fall to `(int)`, which hands 0 to a nullable enum and PHP refuses the
                // call outright (DESIGN §15.88).
                Ty::Opt(inner) => {
                    let one = match inner.as_ref() {
                        Ty::Enum(nm) => format!(
                            "\\{ns}\\{}::from({d}[{k}])",
                            php_name(&self.enum_names.get(nm).cloned().unwrap_or_default())
                        ),
                        Ty::Str => format!("(string) {d}[{k}]"),
                        Ty::Bool => format!("(bool) {d}[{k}]"),
                        Ty::Date => format!("_ord({d}[{k}])"),
                        _ => format!("(int) {d}[{k}]"),
                    };
                    format!("({d}[{k}] === null ? null : {one})")
                }
                _ => format!("(int) {d}[{k}]"),
            });
        }
        // The sequence a walk reads, one element at a time (§15.56).
        let mut seq = String::new();
        if let Some(el) = &self.f.elements {
            let e = php_var(&self.temp("e"));
            let v = php_var(&self.temp("els"));
            let fields: Vec<String> = el
                .fields
                .iter()
                .map(|fd| {
                    let k = php_str(&fd.name.text);
                    match &self.ty_of(&fd.name.text) {
                        Ty::Enum(n) => format!(
                            "\\{ns}\\{}::from({e}[{k}])",
                            php_name(&self.enum_names.get(n).cloned().unwrap_or_default())
                        ),
                        Ty::Str => format!("(string) {e}[{k}]"),
                        Ty::Bool => format!("(bool) {e}[{k}]"),
                        Ty::Date => format!("_ord({e}[{k}])"),
                        // `null` on the wire is PHP's `null`, and the module's parameter is `?Kind`. It
                        // used to fall to `(int)`, which hands 0 to a nullable enum and PHP refuses the
                        // call outright (DESIGN §15.88).
                        Ty::Opt(inner) => {
                            let one = match inner.as_ref() {
                                Ty::Enum(nm) => format!(
                                    "\\{ns}\\{}::from({e}[{k}])",
                                    php_name(&self.enum_names.get(nm).cloned().unwrap_or_default())
                                ),
                                Ty::Str => format!("(string) {e}[{k}]"),
                                Ty::Bool => format!("(bool) {e}[{k}]"),
                                Ty::Date => format!("_ord({e}[{k}])"),
                                _ => format!("(int) {e}[{k}]"),
                            };
                            format!("({e}[{k}] === null ? null : {one})")
                        }
                        _ => format!("(int) {e}[{k}]"),
                    }
                })
                .collect();
            seq = format!(
                "    {v} = [];\n    foreach ({d}[{}] as {e}) {{\n        {v}[] = new \\{ns}\\Element({});\n    }}\n",
                php_str(&el.name.text),
                fields.join(", ")
            );
            args.push(v);
        }
        let a = php_var(&self.temp("args"));
        let (r, trace, line) = (php_var(&self.temp("r")), php_var(&self.temp("trace")), php_var(&self.temp("line")));
        format!(
            "<?php\n\n\
             // Code generated by rulec {}. DO NOT EDIT.\n\n\
             declare(strict_types=1);\n\n\
             // A warning on stdout would land in the middle of the records, so everything the\n\
             // engine has to say goes to stderr whatever the machine's php.ini says.\n\
             ini_set('display_errors', 'stderr');\n\n\
             require_once __DIR__ . '/{alias}.php';\n\n\
             {ord}\
             while (({line} = fgets(STDIN)) !== false) {{\n    \
             {line} = trim({line});\n    \
             if ({line} === '') {{\n        continue;\n    }}\n    \
             {d} = json_decode({line}, true, 512, JSON_THROW_ON_ERROR)['in'];\n\
             {seq}    \
             {a} = [{}];\n    \
             [{r}, {trace}] = \\{ns}\\{fname}_traced(...{a});\n    \
             {a}[] = {r};\n    \
             {a}[] = {trace};\n    \
             echo \\{ns}\\{fname}_record(...{a}), \"\\n\";\n\
             }}\n",
            env!("CARGO_PKG_VERSION"),
            args.join(", "),
            ord = if self.f.inputs.iter().any(|i| matches!(self.ty_of(&i.name.text), Ty::Date))
                || self.element_fields().iter().any(|f| matches!(self.ty_of(&f.name.text), Ty::Date))
            {
                "/// A date as its day number from 1970-01-01, by the same civil-date arithmetic\n\
                 /// the tool uses.\n\
                 function _ord(string $s): int\n{\n    \
                 [$y, $m, $d] = array_map('intval', explode('-', $s));\n    \
                 $y2 = $m <= 2 ? $y - 1 : $y;\n    \
                 $era = intdiv($y2 >= 0 ? $y2 : $y2 - 399, 400);\n    \
                 $yoe = $y2 - $era * 400;\n    \
                 $mp = ($m + 9) % 12;\n    \
                 $doy = intdiv(153 * $mp + 2, 5) + $d - 1;\n    \
                 $doe = $yoe * 365 + intdiv($yoe, 4) - intdiv($yoe, 100) + $doy;\n    \
                 return $era * 146097 + $doe - 719468;\n}\n\n"
            } else {
                ""
            },
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
pub fn round_tests_php() -> String {
    let mut o = format!(
        "<?php\n\n\
         // Code generated by rulec {}. DO NOT EDIT.\n\
         // {}\n\n\
         declare(strict_types=1);\n\n\
         ini_set('display_errors', 'stderr');\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(&round_php());
    o.push_str("\n$cases = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    ['{}', {x}, {g}, {want}],\n", mode_fn(m)));
    }
    o.push_str("];\n\n$bad = 0;\nforeach ($cases as [$mode, $x, $g, $want]) {\n");
    o.push_str("    $got = ('_round_' . $mode)($x, $g);\n    if ($got === $want) {\n        continue;\n    }\n");
    o.push_str("    echo \"NG {$mode}({$x}, {$g}) = {$got}, want {$want}\\n\";\n    $bad++;\n}\n");
    o.push_str("if ($bad > 0) {\n    exit(1);\n}\n");
    o.push_str(&format!("echo {};\n", tr!("'ok ' . count($cases) . \" 件\\n\"", "'ok ' . count($cases) . \" cases\\n\"")));
    o
}

impl<'a> Gen<'a> {
    /// One projected value as the PHP the module takes (§15.125).
    fn php_take(&self, ty: &Ty, e: String) -> String {
        let ns = self.php_ns();
        match ty {
            Ty::Enum(n) => {
                format!("\\{ns}\\{}::from((string) {e})", php_name(&self.enum_names.get(n).cloned().unwrap_or_default()))
            }
            Ty::Str => format!("(string) {e}"),
            Ty::Bool => format!("(bool) {e}"),
            Ty::Date => format!("_days((string) {e})"),
            Ty::Opt(inner) => format!("({e} === null ? null : {})", self.php_take(inner, e.clone())),
            _ => format!("(int) {e}"),
        }
    }

    fn php_proj(&self, p: &super::Proj) -> String {
        if let Some(pp) = &p.proto {
            return self.php_proto(p, pp);
        }
        let walk = p.path.iter().fold(php_var_pub(&p.root), |o, s| format!("{o}[{}]", php_str(s)));
        let cond = || match p.kind.test() {
            Some((fd, c)) => super::elem_cond(
                c,
                &format!("$e[{}]", php_str(&fd.text)),
                &super::Syn { is: "===", isnt: "!==", and: "&&", or: "||", tru: "true", fls: "false", not: None },
            ),
            None => "true".into(),
        };
        match p.kind {
            // `??` reads a missing key, at any depth, as null (§15.132).
            crate::ast::ProjKind::Field if matches!(p.ty, crate::types::Ty::Opt(_)) => self.php_take(&p.ty, format!("({walk} ?? null)")),
            crate::ast::ProjKind::Field => self.php_take(&p.ty, walk),
            crate::ast::ProjKind::Any(..) => format!("count(array_filter({walk}, fn($e) => {})) > 0", cond()),
            crate::ast::ProjKind::All(..) => {
                format!("count(array_filter({walk}, fn($e) => {})) === count({walk})", cond())
            }
            crate::ast::ProjKind::Count(None) => format!("count({walk})"),
            crate::ast::ProjKind::Count(Some(_)) => format!("count(array_filter({walk}, fn($e) => {}))", cond()),
        }
    }

    /// One projection read from a `.proto` contract's JSON form, through `_proto` (§15.133).
    fn php_proto(&self, p: &super::Proj, pp: &crate::projection::ProtoPath) -> String {
        let lit = |z: &crate::projection::Zero| super::pb_zero(z, "false", &php_str);
        let steps: Vec<String> = pp.steps.iter().map(|(j, n)| format!("[{}, {}]", php_str(j), php_str(n))).collect();
        let (absent, unset) = self.proto_defaults(p, pp, &lit, "null");
        let walk = format!("_proto({}, [{}], {absent}, {unset})", php_var_pub(&p.root), steps.join(", "));
        let syn = super::Syn { is: "===", isnt: "!==", and: "&&", or: "||", tru: "true", fls: "false", not: None };
        let cond = || match (p.kind.test(), &pp.elem) {
            (Some((_, c)), Some((j, n, z))) => {
                let at = if j == n {
                    format!("($e[{}] ?? {})", php_str(j), lit(z))
                } else {
                    format!("($e[{}] ?? $e[{}] ?? {})", php_str(j), php_str(n), lit(z))
                };
                // protojson writes a 64-bit integer as a string.
                let at = if *z == crate::projection::Zero::Int { format!("(int) {at}") } else { at };
                super::elem_cond(c, &at, &syn)
            }
            (Some((fd, c)), None) => super::elem_cond(c, &format!("$e[{}]", php_str(&fd.text)), &syn),
            (None, _) => "true".into(),
        };
        match p.kind {
            crate::ast::ProjKind::Field => self.php_take(&p.ty, walk),
            crate::ast::ProjKind::Any(..) => format!("count(array_filter({walk}, fn($e) => {})) > 0", cond()),
            crate::ast::ProjKind::All(..) => format!("count(array_filter({walk}, fn($e) => {})) === count({walk})", cond()),
            crate::ast::ProjKind::Count(None) => format!("count({walk})"),
            crate::ast::ProjKind::Count(Some(_)) => format!("count(array_filter({walk}, fn($e) => {}))", cond()),
        }
    }

    pub(super) fn php_from(&self) -> String {
        if !self.projects() {
            return String::new();
        }
        let fname = self.php_fname();
        let mut params: Vec<String> = self.proj_params().iter().map(|r| format!("array {}", php_var_pub(r))).collect();
        params.extend(
            self.proj_rest()
                .iter()
                .map(|i| format!("{} {}", self.php_ty(&self.ty_of(&i.name.text)), php_var_pub(&pub_name(&i.name)))),
        );
        if let Some(el) = &self.f.elements {
            params.push(format!("array {}", php_var_pub(&pub_name(&el.name))));
        }
        let mut o = String::new();
        if self.proj_proto() {
            o.push_str(
                "function _proto($o, array $path, $absent, $unset)\n{\n    \
                 foreach ($path as $i => [$j, $p]) {\n        \
                 $v = is_array($o) ? ($o[$j] ?? $o[$p] ?? null) : null;\n        \
                 if ($v === null) {\n            \
                 return $i === count($path) - 1 ? $absent : $unset;\n        \
                 }\n        \
                 $o = $v;\n    \
                 }\n    \
                 return $o;\n}\n\n",
            );
        }
        if self.proj_dates() {
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
        o.push_str(&format!("/** {} */\nfunction {fname}_from({})\n{{\n    return {fname}(\n", self.proj_doc(), params.join(", ")));
        let projs = self.projections();
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            args.push(match projs.iter().find(|p| p.jp == i.name.text) {
                Some(p) => self.php_proj(p),
                None => php_var_pub(&pub_name(&i.name)),
            });
        }
        if let Some(el) = &self.f.elements {
            args.push(php_var_pub(&pub_name(&el.name)));
        }
        for a in &args {
            o.push_str(&format!("        {a},\n"));
        }
        o.push_str("    );\n}\n");
        o
    }
}

impl Gen<'_> {
    /// The projection function's signature, for the inventory (§15.125).
    pub(super) fn php_from_signature(&self) -> String {
        let mut params: Vec<String> = self.proj_params().iter().map(|r| format!("array {}", php_var_pub(r))).collect();
        params.extend(
            self.proj_rest()
                .iter()
                .map(|i| format!("{} {}", self.php_ty(&self.ty_of(&i.name.text)), php_var_pub(&super::pub_name(&i.name)))),
        );
        format!("function {}_from({})", self.php_fname(), params.join(", "))
    }
}
