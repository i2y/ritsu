//! The certificate (§15.96): the evidence behind two of the five proofs, in a form a
//! program that shares no code with this one can re-check in milliseconds.
//!
//! What `check` prints is a verdict. A reader who does not want to trust this
//! implementation has, until now, had nothing to hold it to — the proofs are exhaustive,
//! but exhaustive inside a program nobody else has read. Two of them have evidence small
//! enough to hand over:
//!
//! * **no two rows of a `unique` table meet** — for each pair, the one axis on which their
//!   coordinates do not meet. Checking one pair is one set intersection.
//! * **every row is reached** — for each row, a point inside it, given both as coordinates
//!   and as the input it stands for. Checking one row is one lookup.
//!
//! Neither asks the reader to search, which is the difference between evidence and running
//! the same program twice. What is **not** here: completeness (the split tree, a decision of
//! its own), int64, and units. And no certificate covers the step before all of them — that
//! the table in this file is the table the rule's author wrote (§15.47).

use crate::ast::{BinOp, Expr, Item, Lit, RuleFile};
use crate::json::{arr, Obj};
use crate::num::Rat;
use crate::region::{CertCell, CertTable, Cover, certificate_of};
use crate::types::Checked;

/// A rational as the checker reads it: `"7/2"`, or `"3"` when it is whole. An end that is
/// not there is `null`.
fn rat(r: &Rat) -> String {
    if r.den == 1 { r.num.to_string() } else { format!("{}/{}", r.num, r.den) }
}

fn end(r: &Option<Rat>) -> String {
    r.as_ref().map(|v| crate::json::quote(&rat(v))).unwrap_or_else(|| "null".into())
}

/// One expression as a tree the checker can walk. Only the shapes §5 allows appear: a name,
/// a number, the four operators, and a call (a rounding, `min`, `max`).
fn expr_json(e: &Expr, ty: &crate::types::Ty) -> String {
    match e {
        Expr::Name(n, _) => Obj::new().str("name", n).finish(),
        // A literal carries its unit (`10%`, `1万円`), so the value it stands for is
        // resolved here and travels beside the text: the checker does arithmetic, not units.
        Expr::Lit(Lit::Num(n), _) => {
            let v = crate::types::lit_value_in_pub(n, ty).or_else(|| crate::types::lit_value_in_pub(n, &crate::types::lit_ty_pub(n)));
            Obj::new()
                .str("num", &n.raw)
                .raw("value", v.map(|r| crate::json::quote(&rat(&r))).unwrap_or_else(|| "null".into()))
                .str("type", &crate::types::lit_value_in_pub(n, ty).map(|_| ty.to_string()).unwrap_or_else(|| crate::types::lit_ty_pub(n).to_string()))
                .finish()
        }
        // A literal that is not a number: a date, a truth value, a string, or a word whose
        // type only the context gives. The first three carry their own type so a re-checker
        // can derive the units around them; a word does not, and the value it sits in is
        // reported as one whose units were not re-checked rather than passed.
        Expr::Lit(l, _) => {
            let o = Obj::new().str("lit", &format!("{l:?}"));
            match l {
                Lit::Date(..) => o.str("type", "date").finish(),
                Lit::Str(_) => o.str("type", "string").finish(),
                _ => o.finish(),
            }
        }
        Expr::Bin(l, op, r, _) => Obj::new()
            .str(
                "op",
                match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    // A comparison is a claim about units too — `注文金額 >= 3万円` is only
                    // well typed because both sides are 円 — so the operator travels rather
                    // than being flattened to "?" and left unre-checkable (§15.99).
                    BinOp::Le => "<=",
                    BinOp::Ge => ">=",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::Eq => "==",
                },
            )
            .raw("l", expr_json(l, ty))
            .raw("r", expr_json(r, ty))
            .finish(),
        Expr::Call(name, args, _) => Obj::new()
            .str("call", name)
            .raw("args", arr(&args.iter().map(|a| expr_json(a, ty)).collect::<Vec<_>>()))
            .finish(),
    }
}

/// Every named value the rule computes, with the expression, the interval the declared
/// ranges force it into, and the integer that interval stores at its scale (§7.4, E108).
/// Re-checking one is interval arithmetic over the same expression — which is why the
/// ranges travel with it.
fn values_json(f: &RuleFile, c: &Checked) -> Vec<String> {
    // Every value `check` holds to E103 and E108, which is `derive`, `define` **and**
    // `result` — the last of these is neither `Item`, and leaving it out meant a rule whose
    // whole arithmetic is one `result` line stated no values at all (§15.99).
    let mut named: Vec<(&crate::ast::Name, &Expr)> = Vec::new();
    for it in &f.items {
        match it {
            Item::Derived(d) => named.push((&d.name, &d.expr)),
            Item::Define(d) => named.push((&d.name, &d.expr)),
            _ => {}
        }
    }
    let result_name;
    if let Some(r) = &f.result {
        result_name = crate::ast::Name { text: r.name.clone(), ascii: None, span: r.span.clone() };
        named.push((&result_name, &r.expr));
    }
    let mut out = Vec::new();
    for (name, e) in named {
        let Some(sym) = c.syms.get(&name.text) else { continue };
        let obj = Obj::new().str("name", &name.text).str("type", &sym.ty.to_string()).raw("expr", expr_json(e, &sym.ty));
        // A value the tool could not bound is stated with no interval rather than left out.
        // Omitting it hid the value itself: a reader could not tell it existed, and neither
        // re-checker could say that the int64 claim had not been made for it.
        let obj = match (c.interval(e, &sym.ty), c.scale(e)) {
            (Some((lo, hi)), Some(sc)) => {
                let abs = |r: Rat| if r.num < 0 { Rat::zero().sub(r) } else { r };
                let mag = if abs(lo).cmp_to(abs(hi)) == std::cmp::Ordering::Greater { abs(lo) } else { abs(hi) };
                let stored = mag.mul(Rat::int(sc));
                obj.raw("interval", format!("[{},{}]", crate::json::quote(&rat(&lo)), crate::json::quote(&rat(&hi))))
                    .int("scale", sc)
                    .str("stored_max", &(stored.num / stored.den).to_string())
            }
            _ => obj.raw("interval", "null").raw("scale", "null").raw("stored_max", "null"),
        };
        out.push(obj.finish());
    }
    out
}

/// The certificate of one rule, as one JSON object.
/// The bytes a span names, read back out of the file. A span that is not there gives the
/// empty string, and the checker sees the cell as one it could not read.
fn at_span(src: &str, line: usize, col: usize, len: usize) -> String {
    let Some(text) = src.lines().nth(line.wrapping_sub(1)) else { return String::new() };
    if col + len > text.len() || !text.is_char_boundary(col) || !text.is_char_boundary(col + len) {
        return String::new();
    }
    text[col..col + len].to_string()
}

/// Where every cell of a row stands in the file, and what stands there. `null` for a row
/// an `apply` brought in — it is written in another file — and for one cell of a `clause`
/// row the `when` line does not mention.
fn source_json(src: &str, spans: &[Option<(usize, usize, usize)>], tests: &[CertCell]) -> String {
    if spans.is_empty() {
        return "null".into();
    }
    arr(&spans
        .iter()
        .enumerate()
        .map(|(ai, s)| match s {
            None => "null".into(),
            // A cell the row does not have: a merged member without this column, or a
            // column a `clause` does not mention. The span it was padded with points at
            // the row, not at a cell, so the certificate shows nothing rather than that.
            Some((line, col, len))
                if matches!(tests.get(ai), Some(CertCell::Any))
                    && at_span(src, *line, *col, *len) != "-" =>
            {
                "null".into()
            }
            Some((line, col, len)) => Obj::new()
                .int("line", *line as i128)
                .int("col", *col as i128)
                .int("len", *len as i128)
                .str("text", &at_span(src, *line, *col, *len))
                .finish(),
        })
        .collect::<Vec<_>>())
}

pub fn certificate(f: &RuleFile, c: &Checked, src: &str, rule_path: &str) -> String {
    let tables: Vec<String> = c.sets.iter().filter_map(|s| certificate_of(s, c, f).map(|t| table_json(t, src))).collect();
    let contracts: Vec<String> = crate::projection::contract_certificates(f, c, rule_path).iter().map(contract_json).collect();
    let mut ranges = Obj::new();
    let mut names: Vec<&String> = c.ranges.keys().collect();
    names.sort();
    for n in names {
        let (lo, hi) = &c.ranges[n];
        ranges = ranges.raw(n, format!("[{},{}]", end(lo), end(hi)));
    }
    Obj::new()
        .str("rule", &f.name.text)
        .str("alias", f.name.ascii.as_deref().unwrap_or(&f.name.text))
        .str("version", &f.version)
        .str("source_sha256", &crate::sha256::hex(src.as_bytes()))
        .str("rulec", env!("CARGO_PKG_VERSION"))
        .raw("types", {
            let mut ts = Obj::new();
            let mut names: Vec<&String> = c.syms.keys().collect();
            names.sort();
            for n in names {
                ts = ts.str(n, &c.syms[n].ty.to_string());
            }
            ts.finish()
        })
        .raw("ranges", ranges.finish())
        .raw("groups", {
            let mut g = Obj::new();
            let mut names: Vec<&String> = c.groups.keys().collect();
            names.sort();
            for n in names {
                g = g.raw(n, crate::json::strs(&c.groups[n].1));
            }
            g.finish()
        })
        // The guarantees, once for the rule. Every table repeats them where the sieve
        // uses them, but a value's interval can rest on one too — a share is bounded by
        // the amount only because a constraint says the running total stays within the
        // whole (§15.102) — and a rule with no table at all still has values.
        .raw(
            "constraints",
            arr(&f
                .constraints
                .iter()
                .map(|k| {
                    Obj::new().str("left", &k.left).str("op", k.op.word()).str("right", &k.right).finish()
                })
                .collect::<Vec<_>>()),
        )
        .raw("enums", {
            let mut e = Obj::new();
            let mut names: Vec<&String> = c.enums.keys().collect();
            names.sort();
            for n in names {
                e = e.raw(n, crate::json::strs(&c.enums[n]));
            }
            e.finish()
        })
        .raw("values", arr(&values_json(f, c)))
        .raw("tables", arr(&tables))
        .raw("contracts", arr(&contracts))
        .finish()
}

/// One contract's section (§15.142): the condition it places on what the rule reads, as
/// atoms over named values opened into cases, and for each thing the rule's door asks — an
/// end of an input's range, a `constraint`, an enum's values — why every case keeps it. A
/// re-checker builds the door again from the rule; what it takes from here is the reading of
/// the contract and the proofs.
fn contract_json(k: &crate::projection::ContractCert) -> String {
    use crate::relation::{Atom, Proof, Rel};
    let name = |t: &crate::relation::Term| k.names.get(t).cloned().unwrap_or_else(|| format!("@{}", t.word()));
    let atoms: Vec<String> = k
        .atoms
        .iter()
        .map(|a| match a {
            Atom::Num(l, rel) => {
                let mut ts = Obj::new();
                for (t, c) in &l.terms {
                    ts = ts.str(&name(t), &rat(c));
                }
                let r = match rel {
                    Rel::Le => "le",
                    Rel::Lt => "lt",
                    Rel::Eq => "eq",
                    Rel::Ne => "ne",
                };
                Obj::new().raw("num", ts.finish()).str("k", &rat(&l.k)).str("rel", r).finish()
            }
            Atom::Str(t, vs, yes) => Obj::new().str("str", &name(t)).raw("values", crate::json::strs(vs)).bool("in", *yes).finish(),
            Atom::Bool(t, v) => Obj::new().str("bool", &name(t)).bool("value", *v).finish(),
            Atom::Unknown => Obj::new().bool("unknown", true).finish(),
        })
        .collect();
    let proof = |p: &Proof| match p {
        Proof::Clash(t) => Obj::new().str("clash", &name(t)).finish(),
        Proof::Within => Obj::new().bool("within", true).finish(),
        Proof::Farkas(r) => Obj::new()
            .raw(
                "farkas",
                arr(&r
                    .used()
                    .into_iter()
                    .map(|(q, y)| {
                        let o = match &q.origin {
                            crate::fourier::Origin::Contract { atom, part } => Obj::new().int("atom", *atom as i128).int("part", *part as i128),
                            crate::fourier::Origin::None => Obj::new().bool("door", true),
                            _ => Obj::new().str("unknown", ""),
                        };
                        o.str("y", &rat(&y)).finish()
                    })
                    .collect::<Vec<_>>()),
            )
            .finish(),
    };
    let doors: Vec<String> = k
        .doors
        .iter()
        .map(|(d, ps)| {
            let o = match d {
                crate::projection::Door::Range { input, hi } => Obj::new().str("range", input).bool("hi", *hi),
                crate::projection::Door::Constraint(i) => Obj::new().int("constraint", *i as i128),
                crate::projection::Door::Member { input } => Obj::new().str("member", input),
            };
            o.raw("proofs", ps.as_ref().map(|ps| arr(&ps.iter().map(proof).collect::<Vec<_>>())).unwrap_or_else(|| "null".into())).finish()
        })
        .collect();
    Obj::new()
        .str("shape", &k.shape)
        .str("file", &k.file)
        .str("sha256", &k.sha256)
        .bool("unread", k.unread)
        .raw(
            "vars",
            Obj::new()
                .raw("num", crate::json::strs(&k.nums))
                .raw("str", crate::json::strs(&k.strs))
                .raw("bool", crate::json::strs(&k.bools))
                .finish(),
        )
        .raw(
            "inputs",
            arr(&k
                .inputs
                .iter()
                .map(|(n, sc, path)| Obj::new().str("input", n).int("scale", *sc).str("from", path).finish())
                .collect::<Vec<_>>()),
        )
        .raw("atoms", arr(&atoms))
        .raw(
            "cases",
            match &k.cases {
                Some(cs) => arr(&cs.iter().map(|c| arr(&c.iter().map(|i| i.to_string()).collect::<Vec<_>>())).collect::<Vec<_>>()),
                None => "null".into(),
            },
        )
        .raw("doors", arr(&doors))
        .finish()
}

fn table_json(t: CertTable, src: &str) -> String {
    let axes: Vec<String> = t
        .axes
        .iter()
        .map(|a| {
            let bounds: Vec<String> = a
                .bounds
                .iter()
                .map(|b| match b {
                    Some((lo, hi)) => format!("[{},{}]", end(lo), end(hi)),
                    None => "null".into(),
                })
                .collect();
            Obj::new()
                .str("column", &a.name)
                .str("kind", a.kind)
                .raw("coords", crate::json::strs(&a.coords))
                .raw("step", a.step.map(|v| crate::json::quote(&rat(&v))).unwrap_or_else(|| "null".into()))
                .raw(
                    "prefixes",
                    match &a.prefixes {
                        None => "null".into(),
                        Some(ps) => arr(&ps
                            .iter()
                            .map(|p| match p {
                                Some(t) => crate::json::quote(t),
                                None => "null".into(),
                            })
                            .collect::<Vec<_>>()),
                    },
                )
                .raw("bounds", arr(&bounds))
                .finish()
        })
        .collect();
    let rows: Vec<String> = t
        .rows
        .iter()
        .map(|r| {
            let accepts: Vec<String> = r.accepts.iter().map(|xs| arr(&xs.iter().map(|x| x.to_string()).collect::<Vec<_>>())).collect();
            Obj::new()
                .int("row", r.row as i128)
                .str("label", &r.label)
                .raw("cells", crate::json::strs(&r.cells))
                .raw("tests", arr(&r.tests.iter().map(cell_json).collect::<Vec<_>>()))
                .str("origin", &r.origin)
                .int("line", r.line as i128)
                .raw("source", source_json(src, &r.spans, &r.tests))
                .raw("accepts", arr(&accepts))
                .raw(
                    "produces",
                    arr(&r
                        .produces
                        .iter()
                        .map(|p| match p {
                            Some(w) => crate::json::quote(w),
                            None => "null".into(),
                        })
                        .collect::<Vec<_>>()),
                )
                .finish()
        })
        .collect();
    let disjoint: Vec<String> = t
        .disjoint
        .iter()
        .map(|(a, b, ax)| Obj::new().int("a", *a as i128).int("b", *b as i128).int("axis", *ax as i128).finish())
        .collect();
    let undecided: Vec<String> =
        t.undecided.iter().map(|(a, b)| Obj::new().int("a", *a as i128).int("b", *b as i128).finish()).collect();
    let reach: Vec<String> = t
        .reach
        .iter()
        .map(|(row, at, input, nums, extra)| {
            let mut ins = Obj::new();
            for (k, v) in input {
                ins = ins.raw(k, v.json());
            }
            Obj::new()
                .int("row", *row as i128)
                .raw("at", arr(&at.iter().map(|x| x.to_string()).collect::<Vec<_>>()))
                .raw("values", ins.finish())
                .raw(
                    "at_values",
                    arr(&nums
                        .iter()
                        .map(|v| match v {
                            Some(q) => crate::json::quote(&rat(q)),
                            None => "null".into(),
                        })
                        .collect::<Vec<_>>()),
                )
                .raw(
                    "extra_values",
                    arr(&extra
                        .iter()
                        .map(|v| match v {
                            Some(q) => crate::json::quote(&rat(q)),
                            None => "null".into(),
                        })
                        .collect::<Vec<_>>()),
                )
                .finish()
        })
        .collect();
    let refuted: Vec<String> = t
        .refuted
        .iter()
        .map(|(a, b, r)| Obj::new().int("a", *a as i128).int("b", *b as i128).raw("farkas", farkas_json(r, &t.model)).finish())
        .collect();
    Obj::new()
        .str("table", &t.name)
        .str("policy", t.policy)
        .int("outputs", t.outputs as i128)
        .raw("axes", arr(&axes))
        .raw("decides", crate::json::strs(&t.decides))
        .raw("rows", arr(&rows))
        .raw("disjoint", arr(&disjoint))
        .raw("refuted", arr(&refuted))
        .raw("undecided", arr(&undecided))
        .raw("reach", arr(&reach))
        .raw("unused", arr(&t.unused.iter().map(|r| r.to_string()).collect::<Vec<_>>()))
        .raw("unreachable", arr(&t.unreachable.iter().map(|r| r.to_string()).collect::<Vec<_>>()))
        .raw(
            "constraints",
            arr(&t
                .constraints
                .iter()
                .map(|(l, op, r)| Obj::new().str("left", l).str("op", op).str("right", r).finish())
                .collect::<Vec<_>>()),
        )
        .raw(
            "above",
            Obj::new()
                .raw(
                    "never",
                    arr(&t
                        .above
                        .iter()
                        .filter(|f| matches!(f, crate::region::AboveFact::Never { .. }))
                        .map(above_json)
                        .collect::<Vec<_>>()),
                )
                .raw(
                    "apart",
                    arr(&t
                        .above
                        .iter()
                        .filter(|f| matches!(f, crate::region::AboveFact::Apart { .. }))
                        .map(above_json)
                        .collect::<Vec<_>>()),
                )
                .finish(),
        )
        .raw(
            "linear",
            Obj::new()
                .raw("extra", crate::json::strs(&t.model_extra))
                .raw("facts", arr(&t.model.iter().map(fact_json).collect::<Vec<_>>()))
                .finish(),
        )
        .raw("cover", t.cover.as_ref().map(|c| cover_json(c, &t.model)).unwrap_or_else(|| "null".into()))
        .finish()
}

/// One fact of a table's linear model, named by where it comes from, so that a re-checker
/// builds the inequality again from the rule rather than reading it here (§15.141). A
/// `derive`'s equation is two facts, `name − expr <= 0` (`le`) and `>= 0`; a range is two,
/// its low end and its high end; a `constraint` is one, by its place in `constraints`.
fn fact_json(o: &crate::fourier::Origin) -> String {
    use crate::fourier::Origin;
    match o {
        Origin::Derive { name, le } => Obj::new().str("derive", name).bool("le", *le).finish(),
        Origin::Range { name, hi } => Obj::new().str("range", name).bool("hi", *hi).finish(),
        Origin::Constraint(i) => Obj::new().int("constraint", *i as i128).finish(),
        // Nothing else is a fact of the model; a re-checker refuses what it cannot rebuild.
        _ => Obj::new().str("unknown", "").finish(),
    }
}

/// A refutation, as the inequalities that take part and the multiplier of each (§15.139):
/// a fact of the model by its index, or one end of the coordinates the box allows on an
/// axis. Adding them up, each times its multiplier, cancels every name and leaves a constant
/// that is false — which is all a re-checker has to do.
fn farkas_json(r: &crate::fourier::Refutation, model: &[crate::fourier::Origin]) -> String {
    use crate::fourier::Origin;
    arr(&r
        .used()
        .into_iter()
        .map(|(q, y)| {
            let o = match &q.origin {
                // The end itself travels with it — `x >= at`, or `x > at` where it is left
                // out — and a re-checker holds every coordinate of the box to it.
                Origin::Coord { axis, hi } => {
                    let at = if *hi { q.k.mul(Rat::int(-1)) } else { q.k };
                    Obj::new().raw(
                        "coord",
                        Obj::new().int("axis", *axis as i128).bool("hi", *hi).str("at", &rat(&at)).bool("open", q.strict).finish(),
                    )
                }
                other => match model.iter().position(|m| m == other) {
                    Some(i) => Obj::new().int("fact", i as i128),
                    None => Obj::new().str("unknown", ""),
                },
            };
            o.str("y", &rat(&y)).finish()
        })
        .collect::<Vec<_>>())
}

/// The cover, as a tree. An internal node's children are in coordinate order, one per
/// coordinate of the axis at that depth — which is how "the children tile the axis" is
/// checked by shape rather than believed.
fn cover_json(c: &Cover, model: &[crate::fourier::Origin]) -> String {
    match c {
        Cover::Split(kids) => Obj::new().raw("split", arr(&kids.iter().map(|k| cover_json(k, model)).collect::<Vec<_>>())).finish(),
        Cover::ByFarkas(r) => Obj::new().raw("farkas", farkas_json(r, model)).finish(),
        Cover::Row(r) => Obj::new().int("row", *r as i128).finish(),
        Cover::ByConstraint(k) => Obj::new().int("constraint", *k as i128).finish(),
        Cover::ByDerived(ai) => Obj::new().int("derived_axis", *ai as i128).finish(),
        Cover::ByUpstream(what, _) => Obj::new().str("upstream", what).finish(),
        Cover::ByPoints => Obj::new().bool("every_point_ruled_out", true).finish(),
    }
}

/// What the tables above rule out, on this table's own axes.
///
/// A `never` fact says no row of the table that decides the column writes the value at all.
/// An `apart` fact says two coordinates cannot stand together, and carries the reason: the
/// input the two columns share, and the span each of them leaves it. Both are earned back
/// from the rows of the tables that decide the columns, so the numbers here are what a
/// re-checker holds its own arithmetic to, not what it takes on trust (§15.115).
fn above_json(f: &crate::region::AboveFact) -> String {
    use crate::region::{AboveFact, CertSpan};
    let at = |(a, c): &(usize, usize)| Obj::new().int("axis", *a as i128).int("coord", *c as i128).finish();
    let span = |sp: &CertSpan| match sp {
        CertSpan::Num(lo, hi) => format!(
            "[{},{}]",
            lo.map(|v| crate::json::quote(&rat(&v))).unwrap_or_else(|| "null".into()),
            hi.map(|v| crate::json::quote(&rat(&v))).unwrap_or_else(|| "null".into())
        ),
        CertSpan::Words(ws) => crate::json::strs(ws),
    };
    match f {
        AboveFact::Never { axis, coord } => {
            Obj::new().int("axis", *axis as i128).int("coord", *coord as i128).finish()
        }
        AboveFact::Apart { a, b, input, spans } => Obj::new()
            .raw("a", at(a))
            .raw("b", at(b))
            .str("input", input)
            .raw("spans", format!("[{},{}]", span(&spans.0), span(&spans.1)))
            .finish(),
    }
}

/// One cell for the re-checker: the operator and the value its unit stands for, or the
/// words, or nothing at all for a don't-care.
fn cell_json(c: &CertCell) -> String {
    match c {
        CertCell::Any => Obj::new().str("cell", "any").finish(),
        CertCell::Nothing => Obj::new().str("cell", "none").finish(),
        CertCell::Is(ws) => Obj::new().str("cell", "is").raw("words", crate::json::strs(ws)).finish(),
        CertCell::Not(ws) => Obj::new().str("cell", "not").raw("words", crate::json::strs(ws)).finish(),
        CertCell::Prefix(ps) => Obj::new().str("cell", "prefix").raw("words", crate::json::strs(ps)).finish(),
        CertCell::Cmp(cs) => Obj::new()
            .str("cell", "cmp")
            .raw(
                "tests",
                arr(&cs
                    .iter()
                    .map(|(op, v)| {
                        Obj::new()
                            .str("op", op)
                            .raw("value", v.as_ref().map(|r| crate::json::quote(&rat(r))).unwrap_or_else(|| "null".into()))
                            .finish()
                    })
                    .collect::<Vec<_>>()),
            )
            .finish(),
    }
}
