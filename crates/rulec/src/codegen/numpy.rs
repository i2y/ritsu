//! The vectorised target: the rule as data, and one fixed evaluator beside it.
//!
//! Every other backend writes code. This one writes `<alias>.json` — one rule's tables,
//! definitions and result, lowered to integers — and copies `rulec_np.py`, a fixed
//! evaluator that turns a plan into a closure over numpy arrays once, when it is loaded.
//!
//! The shape works because of what rulec refuses. A cell tests its own column and nothing
//! else, so one cell is one comparison over a whole column, a row is the elementwise
//! conjunction of its cells, and a table is a single `np.select`. The proofs pay for the
//! rest: a `unique` table needs no runtime uniqueness test because E105 settled it, the
//! select needs no default because E101 did, and numpy's silently wrapping int64 is safe
//! to compute in because E108 did.
//!
//! The plan is data, so the scale arithmetic of `Gen::expr` is stated a second time here as
//! a tree rather than as text. That duplication is held by the same thing that holds every
//! other backend: `rulec test` runs this one over the vectors and compares its records to
//! the reference evaluator's, byte for byte.

use super::{lcm, pub_name, round_cases, Gen};
use crate::ast::{BinOp, Cell, CmpOp, Expr, Item, Lit, OutCell, Table};
use crate::json::{quote, Obj};
use crate::num::{Rat, RoundMode};
use crate::types::Ty;

/// The evaluator itself, copied beside the plan. It is a checked-in Python file rather than
/// a string built here: it is read by people who never read this crate.
pub fn np_runtime() -> String {
    include_str!("rulec_np.py").to_string()
}

fn int_node(v: i128) -> String {
    format!(r#"{{"k":"int","v":{v}}}"#)
}

fn bin_node(op: &str, l: &str, r: &str) -> String {
    format!(r#"{{"k":"bin","op":"{op}","l":{l},"r":{r}}}"#)
}

/// A node brought from its own scale to `to`, the way [`super::rescale`] does it for text.
fn rescale_node(j: &str, from: i128, to: i128) -> String {
    if from == to {
        j.to_string()
    } else {
        bin_node("*", j, &int_node(to / from))
    }
}

/// What the wire carries for a column, which is what the plan has to decode.
fn kind_of(ty: &Ty) -> &'static str {
    match ty {
        Ty::Opt(t) => kind_of(t),
        Ty::Enum(_) => "enum",
        Ty::Str => "str",
        Ty::Bool => "bool",
        Ty::Date => "date",
        _ => "int",
    }
}

impl<'a> Gen<'a> {
    /// One expression as a tree, with the scale its value is held at. Mirrors `Gen::expr`
    /// case for case; where that one concatenates text, this one nests objects.
    fn np_expr(&self, e: &Expr) -> (String, i128) {
        match e {
            Expr::Name(n, _) => (format!(r#"{{"k":"name","n":{}}}"#, quote(n)), self.scale(n)),
            Expr::Lit(Lit::Num(n), _) => {
                let ty = crate::types::lit_ty_pub(n);
                let v = crate::types::lit_value_in_pub(n, &ty).unwrap_or(Rat::zero());
                let den = v.den.max(1);
                let s = crate::types::lit_scale(n).filter(|s| s % den == 0).unwrap_or(den);
                (int_node(v.num * (s / den)), s)
            }
            Expr::Lit(Lit::Word(w), _) if w == crate::kw::TRUE || w == crate::kw::FALSE => {
                (format!(r#"{{"k":"bool","v":{}}}"#, w == crate::kw::TRUE), 1)
            }
            Expr::Lit(Lit::Date(y, m, d), _) => (int_node(crate::types::date_ord(*y, *m, *d).num), 1),
            Expr::Lit(..) => (int_node(0), 1),
            Expr::Call(name, args, _) => {
                let a: Vec<(String, i128)> = args.iter().map(|x| self.np_expr(x)).collect();
                match (name.as_str(), a.as_slice()) {
                    (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                        let s = lcm(x.1, y.1);
                        let f = if name == crate::kw::MIN { "min" } else { "max" };
                        (
                            format!(
                                r#"{{"k":"fn","f":"{f}","a":[{},{}]}}"#,
                                rescale_node(&x.0, x.1, s),
                                rescale_node(&y.0, y.1, s)
                            ),
                            s,
                        )
                    }
                    (m, [x, g]) if RoundMode::parse(m).is_some() => {
                        let s = lcm(x.1, g.1);
                        let rounded = format!(
                            r#"{{"k":"round","mode":"{m}","x":{},"g":{}}}"#,
                            rescale_node(&x.0, x.1, s),
                            rescale_node(&g.0, g.1, s)
                        );
                        if s == g.1 {
                            (rounded, s)
                        } else {
                            (bin_node("//", &rounded, &int_node(s / g.1)), g.1)
                        }
                    }
                    _ => (int_node(0), 1),
                }
            }
            Expr::Bin(l, op, r, _) => {
                let a = self.np_expr(l);
                let b = self.np_expr(r);
                use BinOp::*;
                match op {
                    Add | Sub => {
                        let s = lcm(a.1, b.1);
                        let o = if *op == Add { "+" } else { "-" };
                        (bin_node(o, &rescale_node(&a.0, a.1, s), &rescale_node(&b.0, b.1, s)), s)
                    }
                    Mul => (bin_node("*", &a.0, &b.0), a.1 * b.1),
                    Div => {
                        // Dividing by a constant leaves the stored integer alone and widens
                        // the scale; E115 is what keeps the other branch unreachable.
                        let k = int_of(&a, &b);
                        match k {
                            Some(k) => (a.0, a.1 * (k / b.1)),
                            None => (bin_node("//", &a.0, &b.0), a.1),
                        }
                    }
                    Le | Lt | Ge | Gt | Eq => {
                        let s = lcm(a.1, b.1);
                        let o = match op {
                            Le => "<=",
                            Lt => "<",
                            Ge => ">=",
                            Gt => ">",
                            _ => "==",
                        };
                        (bin_node(o, &rescale_node(&a.0, a.1, s), &rescale_node(&b.0, b.1, s)), 1)
                    }
                }
            }
        }
    }

    /// One cell as a test on its own column. `None` is `-`, which the plan leaves out.
    fn np_cell(&self, cell: &Cell, col: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let c = quote(col);
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => quote(w),
                Lit::Num(n) => {
                    let v = crate::types::lit_value_in_pub(n, inner).unwrap_or(Rat::zero());
                    format!("{}", v.mul(Rat::int(col_scale)).num)
                }
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => quote(s),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| quote(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("[{}]", out.join(","))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!(r#"{{"col":{c},"op":"eq","v":{}}}"#, quote(crate::kw::NONE)),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                let (_, ms) = &self.c.groups[w];
                format!(
                    r#"{{"col":{c},"op":"in","vals":[{}]}}"#,
                    ms.iter().map(|m| quote(m)).collect::<Vec<_>>().join(",")
                )
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => format!(r#"{{"col":{c},"op":"true"}}"#),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!(r#"{{"col":{c},"op":"false"}}"#),
            Cell::Lit(l) => format!(r#"{{"col":{c},"op":"eq","v":{}}}"#, lit(l)),
            Cell::Set(ls) => format!(r#"{{"col":{c},"op":"in","vals":{}}}"#, members(ls)),
            Cell::Not(ls) => format!(r#"{{"col":{c},"op":"notin","vals":{}}}"#, members(ls)),
            Cell::Cmp(cs) => format!(
                r#"{{"col":{c},"op":"cmp","tests":[{}]}}"#,
                cs.iter()
                    .map(|(o, l)| {
                        let op = match o {
                            CmpOp::Le => "<=",
                            CmpOp::Ge => ">=",
                            CmpOp::Lt => "<",
                            CmpOp::Gt => ">",
                        };
                        format!("[\"{op}\",{}]", lit(l))
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        })
    }

    fn np_table(&self, t: &Table) -> String {
        let rows: Vec<String> = t
            .rows
            .iter()
            .enumerate()
            .map(|(_ri, row)| {
                let tests: Vec<String> = t
                    .inputs
                    .iter()
                    .enumerate()
                    .filter_map(|(ci, (col, _))| {
                        let ty = self.ty_of(col);
                        self.np_cell(row.cells.get(ci)?, col, &ty, self.scale(col))
                    })
                    .collect();
                let vals: Vec<String> = t
                    .outputs
                    .iter()
                    .enumerate()
                    .map(|(oi, oc)| {
                        let name = &oc.name.text;
                        let ty = self.ty_of(name);
                        let scale = self.scale(name);
                        match row.outs.get(oi) {
                            Some(OutCell::Lit(Lit::Num(n))) => {
                                let v = crate::types::lit_value_in_pub(n, &ty).unwrap_or(Rat::zero());
                                format!("{}", v.mul(Rat::int(scale)).num)
                            }
                            Some(OutCell::Lit(Lit::Word(w))) if w == crate::kw::TRUE => "true".into(),
                            Some(OutCell::Lit(Lit::Word(w))) if w == crate::kw::FALSE => "false".into(),
                            Some(OutCell::Lit(Lit::Word(w))) => quote(w),
                            Some(OutCell::Lit(Lit::Date(y, m, d))) => {
                                format!("{}", crate::types::date_ord(*y, *m, *d).num)
                            }
                            Some(OutCell::Lit(Lit::Str(s))) => quote(s),
                            Some(OutCell::Name(w)) if w == crate::kw::TRUE => "true".into(),
                            Some(OutCell::Name(w)) if w == crate::kw::FALSE => "false".into(),
                            // A cell that names a value of the column's enum is that value; a
                            // cell that names a declaration is read at its own scale and
                            // widened to the column's, the way `rescaled` does for text.
                            Some(OutCell::Name(w)) if self.value_names.contains_key(w) => quote(w),
                            Some(OutCell::Name(w)) => {
                                let (a, b) = (self.scale(w), scale);
                                let n = format!(r#"{{"k":"name","n":{}}}"#, quote(w));
                                if a == b || a == 0 || b % a != 0 {
                                    n
                                } else {
                                    bin_node("*", &n, &int_node(b / a))
                                }
                            }
                            None => "0".into(),
                        }
                    })
                    .collect();
                let (tn, rn, label) = self.fired_of(t, _ri);
                let mut fired = Obj::new().str("table", &tn).int("row", rn as i128);
                if !label.is_empty() {
                    fired = fired.str("label", &label);
                }
                format!(
                    r#"{{"tests":[{}],"vals":[{}],"fired":{}}}"#,
                    tests.join(","),
                    vals.join(","),
                    fired.finish()
                )
            })
            .collect();
        Obj::new()
            .str("op", "table")
            .raw("outs", crate::json::strs(&t.outputs.iter().map(|o| o.name.text.clone()).collect::<Vec<_>>()))
            .raw(
                "kinds",
                crate::json::strs(
                    &t.outputs.iter().map(|o| kind_of(&self.ty_of(&o.name.text)).to_string()).collect::<Vec<_>>(),
                ),
            )
            .raw("rows", format!("[{}]", rows.join(",")))
            .finish()
    }

    /// The rule as the plan `rulec_np.py` reads.
    pub fn np_plan(&self) -> String {
        let mut inputs: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let name = &i.name.text;
            let ty = self.ty_of(name);
            let kind = kind_of(&ty);
            let mut o = Obj::new().str("name", name).str("kind", kind).int("scale", self.scale(name));
            let inner = match &ty {
                Ty::Opt(t) => t.as_ref(),
                other => other,
            };
            if let Ty::Enum(en) = inner {
                if let Some(vs) = self.c.enums.get(en) {
                    o = o.str("enum", en).raw("values", crate::json::strs(vs));
                }
            }
            if kind == "int" || kind == "date" {
                if let Some((lo, hi)) = self.c.ranges.get(name) {
                    let s = self.scale(name);
                    if let Some(l) = lo {
                        o = o.int("min", l.mul(Rat::int(s)).num);
                    }
                    if let Some(h) = hi {
                        o = o.int("max", h.mul(Rat::int(s)).num);
                    }
                }
            }
            inputs.push(o.finish());
        }

        let mut steps: Vec<String> = Vec::new();
        for it in &self.f.items {
            match it {
                // The unit is the definition set (§15.66): every table that defines one
                // output is merged into one, exceptions first, and `table_at` hands it back
                // at the member where it is evaluated. Walking the members separately is how
                // a clause that did not fire still wrote its row into the trace.
                Item::Table(t) => {
                    if let Some(t) = self.c.table_at(t) {
                        steps.push(self.np_table(t));
                    }
                }
                Item::Derived(d) => {
                    let (e, _) = self.np_expr(&d.expr);
                    steps.push(Obj::new().str("op", "define").str("name", &d.name.text).raw("expr", e).finish());
                }
                Item::Define(d) => {
                    let (e, _) = self.np_expr(&d.expr);
                    steps.push(Obj::new().str("op", "define").str("name", &d.name.text).raw("expr", e).finish());
                }
                Item::Count(_) => {}
            }
        }

        // Every output: its source, then its one rounding — the same two steps the other
        // eleven emit, with the division that brings the raw scale back to the wire's.
        let mut outputs: Vec<String> = Vec::new();
        for (oi, od) in self.f.outputs.iter().enumerate() {
            let name = &od.name.text;
            let ty = self.ty_of(name);
            let (expr, scale) = match (&self.f.result, oi) {
                (Some(r), 0) => self.np_expr(&r.expr),
                _ => (format!(r#"{{"k":"name","n":{}}}"#, quote(name)), self.scale(name)),
            };
            let os = self.out_scale(name);
            let mut o = Obj::new().str("name", name).str("kind", kind_of(&ty)).raw("expr", expr);
            if let Some(rd) = &od.rounding {
                let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                let grid = g.num * scale / g.den;
                o = o.raw(
                    "round",
                    Obj::new()
                        .str("mode", &rd.mode)
                        .int("grid", grid)
                        .int("div", if scale == os { 1 } else { scale / os })
                        .finish(),
                );
            }
            outputs.push(o.finish());
        }

        Obj::new()
            .str("rule", &self.f.name.text)
            .str("alias", pub_name(&self.f.name))
            .str("version", &self.f.version)
            .str("sha256", &self.src_hash)
            .raw("inputs", format!("[{}]", inputs.join(",")))
            .raw("outputs", format!("[{}]", outputs.join(",")))
            .raw("steps", format!("[{}]", steps.join(",")))
            .finish()
    }

    /// The runner `rulec test` drives: it reads the whole file, decides it in one pass, and
    /// writes the records back in order. Reading every line first is the point — a runner
    /// that called the rule per line would answer correctly and prove nothing.
    pub fn np_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let body = format!(
            r#"import json
import sys

import numpy as np
import rulec_np

rule = rulec_np.load("{alias}.json")
rows = [json.loads(l) for l in sys.stdin if l.strip()]
cols = {{i: [r["in"][i] for r in rows] for i in rule.inputs}}


def _wire(v):
    if isinstance(v, np.bool_):
        return bool(v)
    if isinstance(v, np.integer):
        return int(v)
    return str(v)


try:
    out, fired = rule.traced(**cols)
except rulec_np.RuleInputError as e:
    print(f"error: {{e}}", file=sys.stderr)
    sys.exit(1)

for n, r in enumerate(rows):
    trace = []
    for picked, rows_ in fired:
        trace.append(rows_[int(picked[n])])
    rec = {{
        "in": r["in"],
        "observed": {{k: _wire(v[n]) for k, v in out.items()}},
        "trace": trace,
    }}
    print(json.dumps(rec, ensure_ascii=False, separators=(",", ":")))
"#
        );
        format!("{}{body}", self.header("#"))
    }

    /// What `rulec api` says about this target: the two files, how the plan is loaded, and
    /// the column each input and output is, so that a caller never has to read the plan.
    pub(super) fn api_numpy(&self) -> String {
        let alias = pub_name(&self.f.name);
        let np_ty = |ty: &Ty| -> &'static str {
            match kind_of(ty) {
                "bool" => "ndarray[bool]",
                "int" | "date" => "ndarray[int64]",
                _ => "ndarray[str]",
            }
        };
        let columns: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &i.name.text, np_ty(&ty), &ty)
            })
            .collect();
        let outputs: Vec<String> = self
            .f
            .outputs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &od.name.text, np_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        Obj::new()
            .str("plan", format!("{alias}.json"))
            .str("runtime", "rulec_np.py")
            .str("load", format!("rule = rulec_np.load(\"{alias}.json\")"))
            .str("call", "rule(**{column: sequence}) -> {output: ndarray}")
            .str("traced", "rule.traced(**{column: sequence}) -> (outputs, fired)")
            .str(
                "wire",
                &tr!(
                    "列ごとに同じ長さの並びを渡す。値は生成コードと同じワイヤ形式（宣言した単位の整数、率は刻みの個数、日付は YYYY-MM-DD、列挙はその名前）。",
                    "One equal-length sequence per column. The values are the wire format the generated code takes: an integer in the declared unit, a rate as the count of steps, a date as YYYY-MM-DD, an enum as its name."
                ),
            )
            .raw("columns", crate::json::arr(&columns))
            .raw("outputs", crate::json::arr(&outputs))
            .raw("needs", crate::json::strs(&["numpy"]))
            .finish()
    }
}

/// The constant on the right of a division, when it is one: the same test `Gen::expr` makes.
fn int_of(_a: &(String, i128), b: &(String, i128)) -> Option<i128> {
    let v: i128 = b.0.strip_prefix(r#"{"k":"int","v":"#)?.strip_suffix('}')?.parse().ok()?;
    if b.1 > 0 && v % b.1 == 0 { Some(v) } else { None }
}

pub fn round_tests_numpy() -> String {
    let mut o = format!(
        "# Code generated by rulec {}. DO NOT EDIT.\n\
         # {}\n\
         import sys\n\nimport numpy as np\nimport rulec_np\n\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の五モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The five modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str("CASES = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", super::mode_fn(m)));
    }
    o.push_str(
        "]\n\nFN = {\n    \"down\": rulec_np._round_down,\n    \"up\": rulec_np._round_up,\n    \
         \"half\": rulec_np._round_half,\n    \"half_down\": rulec_np._round_half_down,\n    \
         \"bankers\": rulec_np._round_bankers,\n}\n\n\
         bad = 0\n\
         for mode, x, g, want in CASES:\n    \
             got = int(FN[mode](np.array([x], dtype=np.int64), np.int64(g))[0])\n    \
             if got != want:\n        \
                 print(f\"NG {mode}({x}, {g}) = {got}, want {want}\")\n        \
                 bad += 1\n\
         if bad:\n    sys.exit(1)\n",
    );
    o.push_str(&format!("print(f\"{}\")\n", tr!("ok {{len(CASES)}} 件", "ok {{len(CASES)}} cases")));
    o
}
