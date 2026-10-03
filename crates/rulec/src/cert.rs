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
        // The version of this shape. A program that re-checks a certificate reads it first and
        // refuses one it was not written for, rather than checking something else (§15.156).
        .int("v", 1)
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
        .raw("machine", machine_json(f, c, src))
        .finish()
}

/// A machine's section (§15.148): the claims about every sequence of calls, laid on the
/// certificate of the table that decides the carried state.
///
/// The transitions are read off that table's **rows**, not its cover: a row that answers a
/// call from a state takes that state on the state's axis, so "from `s`, this row goes to
/// `next(r, s)`" for every row whose box takes `s` includes every transition there is — more
/// than there is, when a row's box takes a state no call from that state reaches it with.
/// Over that relation the claims that nothing bad is reached (`final`, `never`, `once`) are a
/// closed set containing the start and nothing bad, which a re-checker confirms by stepping
/// once from each member. That a final state can still be reached is the other direction —
/// a transition has to really happen — so each step of those paths comes with a call: a point
/// in the row's box, at that state, with the values behind it, as a reach point does.
///
/// Where the rows alone cannot carry a claim that `check` proved over the cells — a row whose
/// box takes a state it is never reached from — the claim is listed as not certified here,
/// with why, rather than stated.
fn machine_json(f: &RuleFile, c: &Checked, src: &str) -> String {
    use crate::ast::{Cell, OutCell, Policy};
    let Some(m) = &f.machine else { return "null".into() };
    let Some((cin, cout)) = m.carried() else { return "null".into() };
    let Some(crate::types::Ty::Enum(en)) = c.ty_of(cin) else { return "null".into() };
    let states: Vec<String> = c.enums.get(&en).cloned().unwrap_or_default();
    let idx = |v: &str| states.iter().position(|x| x == v);
    let (Some(init), finals) = (m.initial.as_ref().and_then(|(v, _)| idx(&v.text)), m.finals.iter().filter_map(|v| idx(&v.text)).collect::<Vec<_>>()) else {
        return "null".into();
    };
    let not = |why: String| Obj::new().str("table", "").str("why", &why).finish();
    let Some(set) = c.sets.iter().find(|s| s.table.outputs.iter().any(|o| o.name.text == cout)) else {
        return not(tr!("持ち越す出力を決める表がありません", "no table decides the carried output"));
    };
    if set.merged() {
        return not(tr!(
            "持ち越す出力を二つ以上の表が決めているので、行き先を一つの表の行から読めません",
            "several tables decide the carried output, so the transitions cannot be read off one table's rows"
        ));
    }
    let (Some(ct), Some(reg)) = (certificate_of(set, c, f), crate::region::region_of(set, c, f)) else {
        return not(tr!("遷移を決める表に証明書がありません", "the table that decides the transitions has no certificate"));
    };
    let t = &set.table;
    let unique = t.policy == Policy::Unique;
    let axis = ct.axes.iter().position(|a| a.name == cin);
    if let Some(k) = axis {
        if ct.axes[k].coords != states {
            return not(tr!("状態の軸が列挙の値の並びと違います", "the state's axis is not the enum's values in order"));
        }
    }
    let n = states.len();
    let jout = t.outputs.iter().position(|o| o.name.text == cout).unwrap_or(0);
    // Each row's move, and its box on the state's axis.
    enum Move {
        To(usize),
        Stay,
    }
    let mut moves: Vec<(usize, Move, Vec<usize>)> = Vec::new();
    let mut rows_json: Vec<String> = Vec::new();
    for (ri, r) in t.rows.iter().enumerate() {
        let on: Vec<usize> = match axis {
            Some(k) => ct.rows.get(ri).and_then(|cr| cr.accepts.get(k).cloned()).unwrap_or_default(),
            None => (0..n).collect(),
        };
        let span = r.out_spans.get(jout).map(|sp| (sp.line, sp.col, sp.len));
        // A value of the state's enum is written as a word, which the parser keeps as a name.
        let word = match r.outs.get(jout) {
            Some(OutCell::Lit(Lit::Word(w))) | Some(OutCell::Name(w)) => Some(w.as_str()),
            _ => None,
        };
        let (mv, o) = match word {
            Some(w) if w == cin => (Move::Stay, Obj::new().int("row", r.index as i128).bool("stay", true)),
            Some(w) if idx(w).is_some() => {
                let to = idx(w).unwrap_or(0);
                (Move::To(to), Obj::new().int("row", r.index as i128).int("to", to as i128))
            }
            _ => {
                return not(tr!(
                    "表 {} 行{} は次の状態を計算で決めているので、行から行き先を読めません",
                    "table {} row {} computes the next state, so its move cannot be read off the row",
                    t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                    r.index
                ))
            }
        };
        let o = match span {
            Some((l, col, len)) => o.raw(
                "source",
                Obj::new().int("line", l as i128).int("col", col as i128).int("len", len as i128).str("text", &at_span(src, l, col, len)).finish(),
            ),
            None => o,
        };
        rows_json.push(o.finish());
        moves.push((r.index, mv, on));
    }
    let target = |mv: &Move, st: usize| match mv {
        Move::To(x) => *x,
        Move::Stay => st,
    };
    // The worlds (§15.149). A case holds its `held` inputs, so where the table reads one as a
    // column, a case only ever takes the rows that take its coordinate there. Every claim is
    // laid on the rows once per world: a combination of coordinates of those columns.
    let fixed_axes: Vec<usize> = m.held.iter().filter_map(|x| ct.axes.iter().position(|a| a.name == x.text)).collect();
    let mut worlds: Vec<Vec<usize>> = vec![Vec::new()];
    for &k in &fixed_axes {
        let nk = ct.axes[k].coords.len();
        worlds = worlds.into_iter().flat_map(|w| (0..nk).map(move |ck| [w.clone(), vec![ck]].concat())).collect();
    }
    let takes = |w: &[usize], ri: usize| -> bool {
        fixed_axes.iter().zip(w).all(|(&k, &ck)| ct.rows.get(ri).and_then(|cr| cr.accepts.get(k)).is_some_and(|acc| acc.contains(&ck)))
    };
    let in_world = |w: &[usize]| -> String {
        if w.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = fixed_axes.iter().zip(w).map(|(&k, &ck)| format!("{} = {}", ct.axes[k].name, ct.axes[k].coords[ck])).collect();
        tr!("、{} のとき", ", where {}", parts.join(", "))
    };
    // A fixed input that something else computes with, or that a constraint names: one value of
    // it may allow calls another value of the same coordinate does not, so a path of calls is
    // not shown to be one case's.
    let coupled = coupled_fixed(f, m, &ct);
    let mut uncertified: Vec<String> = Vec::new();

    // What each `never` and `once` line says, the same in every world.
    let never_defs: Vec<(Vec<usize>, Vec<usize>)> = m
        .nevers
        .iter()
        .map(|nv| (nv.states.iter().filter_map(|x| idx(&x.text)).collect(), nv.after.iter().filter_map(|x| idx(&x.text)).collect()))
        .collect();
    let mut once_defs: Vec<String> = Vec::new();
    let mut once_counts: Vec<Option<Vec<(usize, bool)>>> = Vec::new();
    for on in &m.onces {
        let Some(jo) = t.outputs.iter().position(|o| o.name.text == on.output.text) else {
            uncertified.push(tr!(
                "`once {}` の行（{} 行目。その出力は遷移を決める表の外で決まる）",
                "the `once {}` line (line {}: the output is decided outside the table that decides the transitions)",
                on.output.text,
                on.span.line
            ));
            once_defs.push(Obj::new().str("output", &on.output.text).finish());
            once_counts.push(None);
            continue;
        };
        let oty = c.ty_of(&on.output.text).unwrap_or(crate::types::Ty::Unknown);
        let scale = c.wire_scale(&on.output.text);
        // The test in the wire's own words: an integer interval, or a set of words.
        let test = match &on.cell {
            Cell::Cmp(ops) => {
                let (mut lo, mut hi): (Option<i128>, Option<i128>) = (None, None);
                for (op, l) in ops {
                    let Lit::Num(nm) = l else { continue };
                    let Some(v) = crate::types::lit_value_in_pub(nm, &oty) else { continue };
                    let w = crate::types::wire_int(v, scale);
                    match op {
                        crate::ast::CmpOp::Ge => lo = Some(lo.map_or(w, |x| x.max(w))),
                        crate::ast::CmpOp::Gt => lo = Some(lo.map_or(w + 1, |x| x.max(w + 1))),
                        crate::ast::CmpOp::Le => hi = Some(hi.map_or(w, |x| x.min(w))),
                        crate::ast::CmpOp::Lt => hi = Some(hi.map_or(w - 1, |x| x.min(w - 1))),
                    }
                }
                Obj::new()
                    .raw("lo", lo.map(|x| x.to_string()).unwrap_or_else(|| "null".into()))
                    .raw("hi", hi.map(|x| x.to_string()).unwrap_or_else(|| "null".into()))
                    .finish()
            }
            other => {
                let words: Vec<String> = match other {
                    Cell::Lit(Lit::Word(w)) => vec![w.clone()],
                    Cell::Set(ls) => ls.iter().filter_map(|l| if let Lit::Word(w) = l { Some(w.clone()) } else { None }).collect(),
                    _ => Vec::new(),
                };
                Obj::new().raw("words", crate::json::strs(&words)).finish()
            }
        };
        let mut flags: Vec<(usize, bool)> = Vec::new();
        let mut rows_o: Vec<String> = Vec::new();
        for r in &t.rows {
            let lit = match r.outs.get(jo) {
                Some(OutCell::Lit(l)) => Some(l.clone()),
                // A value word (an enum's value, `true`) is kept as a name by the parser.
                Some(OutCell::Name(w))
                    if matches!(oty, crate::types::Ty::Enum(_) | crate::types::Ty::Bool)
                        && crate::eval::lit_to_val(&Lit::Word(w.clone()), &oty).is_some() =>
                {
                    Some(Lit::Word(w.clone()))
                }
                _ => None,
            };
            let (value, counts) = match lit.as_ref() {
                Some(l) => match crate::eval::lit_to_val(l, &oty) {
                    Some(v) => {
                        let wv = crate::eval::wval(c, &on.output.text, &v);
                        (Some(wv.json()), crate::eval::cell_matches(c, &on.cell, &v, &oty))
                    }
                    None => (None, true),
                },
                // A value computed from the call is counted wherever it might count.
                _ => (None, true),
            };
            flags.push((r.index, counts));
            let mut o = Obj::new().int("row", r.index as i128).raw("value", value.unwrap_or_else(|| "null".into()));
            if let Some(sp) = r.out_spans.get(jo) {
                o = o.raw(
                    "source",
                    Obj::new()
                        .int("line", sp.line as i128)
                        .int("col", sp.col as i128)
                        .int("len", sp.len as i128)
                        .str("text", &at_span(src, sp.line, sp.col, sp.len))
                        .finish(),
                );
            }
            rows_o.push(o.finish());
        }
        once_defs.push(Obj::new().str("output", &on.output.text).raw("test", test).raw("rows", arr(&rows_o)).finish());
        once_counts.push(Some(flags));
    }

    let mut worlds_json: Vec<String> = Vec::new();
    let mut finish_coupled_said = false;
    for w in &worlds {
        // One step from a state, over every row of this world whose box takes it.
        let step = |st: usize| -> Vec<(usize, usize)> {
            moves
                .iter()
                .enumerate()
                .filter(|(ri, (_, _, on))| on.contains(&st) && takes(w, *ri))
                .map(|(_, (rindex, mv, _))| (*rindex, target(mv, st)))
                .collect()
        };
        let closure = |start: Vec<(usize, u8)>, next: &dyn Fn(usize, u8, usize, usize) -> u8| -> Vec<(usize, u8)> {
            let mut seen = start.clone();
            let mut q = std::collections::VecDeque::from(start);
            while let Some((st, b)) = q.pop_front() {
                for (ri, to) in step(st) {
                    let nb = next(st, b, ri, to);
                    if !seen.contains(&(to, nb)) {
                        seen.push((to, nb));
                        q.push_back((to, nb));
                    }
                }
            }
            seen
        };
        let reach: Vec<usize> = {
            let mut r: Vec<usize> = closure(vec![(init, 0)], &|_, _, _, _| 0).into_iter().map(|(x, _)| x).collect();
            r.sort_unstable();
            r
        };
        // `final`: nothing leads out of a final state a case reaches in this world.
        let mut final_ok = !finals.is_empty();
        for &fs in finals.iter().filter(|fs| reach.contains(fs)) {
            if step(fs).iter().any(|(_, to)| *to != fs) {
                final_ok = false;
                uncertified.push(tr!(
                    "終わりの状態 {} から出る行がある{}（その状態からは当たらないことを、行だけでは示せない）",
                    "a row leads out of the final state {}{} (that no call from it reaches the row cannot be shown from the rows alone)",
                    states[fs],
                    in_world(w)
                ));
            }
        }
        // `never A after B`.
        let mut nevers: Vec<String> = Vec::new();
        for (nv, (a, b)) in m.nevers.iter().zip(&never_defs) {
            let start = (init, u8::from(b.contains(&init)));
            let set = closure(vec![start], &|_, seen, _, to| u8::from(seen == 1 || b.contains(&to)));
            if set.iter().any(|(st, seen)| *seen == 1 && a.contains(st)) {
                uncertified.push(tr!("`never` の行（{} 行目{}）", "the `never` line (line {}{})", nv.span.line, in_world(w)));
                nevers.push("null".into());
            } else {
                nevers.push(arr(&set.iter().map(|(st, sn)| format!("[{st},{}]", *sn == 1)).collect::<Vec<_>>()));
            }
        }
        // `once`.
        let mut onces: Vec<String> = Vec::new();
        for (on, fl) in m.onces.iter().zip(&once_counts) {
            let Some(flags) = fl else {
                onces.push("null".into());
                continue;
            };
            let counts = |ri: usize| flags.iter().any(|(x, y)| *x == ri && *y);
            let set = closure(vec![(init, 0)], &|_, n, ri, _| (n + u8::from(counts(ri))).min(2));
            if set.iter().any(|(_, k)| *k >= 2) {
                uncertified.push(tr!("`once {}` の行（{} 行目{}）", "the `once {}` line (line {}{})", on.output.text, on.span.line, in_world(w)));
                onces.push("null".into());
            } else {
                onces.push(arr(&set.iter().map(|(st, k)| format!("[{st},{k}]")).collect::<Vec<_>>()));
            }
        }
        // A final state can still be reached: paths of calls that really happen, each made
        // with this world's coordinates.
        let mut finish: Vec<String> = Vec::new();
        if !finals.is_empty() && !coupled.is_empty() {
            if !finish_coupled_said {
                finish_coupled_said = true;
                uncertified.push(tr!(
                    "終わりの状態へ行く呼び出しの並び（遷移を決める表が入力でない値を読んでいるか、制約が {} を名指ししていて、一つの値のまま通れるかを行から示せない）",
                    "the sequences of calls to a final state (the table that decides the transitions reads a value that is not an input, or a constraint names {}, so that one value carries a case through is not shown from the rows)",
                    coupled.join(", ")
                ));
            }
        } else if !finals.is_empty() {
            // Which (state, row) steps a call really makes, with the call.
            let mut witnessed: Vec<(usize, usize, usize, String)> = Vec::new();
            for (ri, (rindex, mv, on)) in moves.iter().enumerate() {
                if !takes(w, ri) {
                    continue;
                }
                for &st in on {
                    let mut fix: Vec<(usize, usize)> = fixed_axes.iter().copied().zip(w.iter().copied()).collect();
                    let point = match axis {
                        Some(k) => {
                            fix.push((k, st));
                            reg.point_at_all(ri, unique, &fix)
                        }
                        None if fix.is_empty() => ct.reach.iter().find(|x| x.0 == *rindex).map(|x| (x.1.clone(), x.2.clone(), x.3.clone(), x.4.clone())),
                        None => reg.point_at_all(ri, unique, &fix),
                    };
                    let Some((at, input, nums, extra)) = point else { continue };
                    let mut ins = Obj::new();
                    for (k2, v) in &input {
                        ins = ins.raw(k2, v.json());
                    }
                    let q = |v: &Vec<Option<Rat>>| arr(&v.iter().map(|x| x.map(|y| crate::json::quote(&rat(&y))).unwrap_or_else(|| "null".into())).collect::<Vec<_>>());
                    let o = Obj::new()
                        .int("state", st as i128)
                        .int("row", *rindex as i128)
                        .raw("at", arr(&at.iter().map(|x| x.to_string()).collect::<Vec<_>>()))
                        .raw("values", ins.finish())
                        .raw("at_values", q(&nums))
                        .raw("extra_values", q(&extra))
                        .finish();
                    witnessed.push((st, *rindex, target(mv, st), o));
                }
            }
            // Backward from the final states over the witnessed steps.
            let mut via: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
            let mut done: Vec<usize> = finals.clone();
            loop {
                let before = done.len();
                for (i, (st, _, to, _)) in witnessed.iter().enumerate() {
                    if done.contains(to) && !done.contains(st) {
                        done.push(*st);
                        via.insert(*st, i);
                    }
                }
                if done.len() == before {
                    break;
                }
            }
            for &st in &reach {
                if finals.contains(&st) {
                    continue;
                }
                if !done.contains(&st) {
                    uncertified.push(tr!(
                        "{} から終わりの状態へ行く呼び出しの並び{}",
                        "a sequence of calls from {} to a final state{}",
                        states[st],
                        in_world(w)
                    ));
                    continue;
                }
                let mut path: Vec<String> = Vec::new();
                let mut at = st;
                while !finals.contains(&at) {
                    let Some(&i) = via.get(&at) else { break };
                    path.push(witnessed[i].3.clone());
                    at = witnessed[i].2;
                }
                finish.push(Obj::new().int("state", st as i128).raw("path", arr(&path)).finish());
            }
        }
        worlds_json.push(
            Obj::new()
                .raw("at", arr(&w.iter().map(|x| x.to_string()).collect::<Vec<_>>()))
                .raw("reach", arr(&reach.iter().map(|x| x.to_string()).collect::<Vec<_>>()))
                // Whether "nothing leads out of a final state" is laid on the rows here.
                .bool("final_certified", final_ok)
                .raw("never", arr(&nevers))
                .raw("once", arr(&onces))
                .raw("finish", arr(&finish))
                .finish(),
        );
    }
    Obj::new()
        .str("table", t.name.as_ref().map(|n| n.text.as_str()).unwrap_or(""))
        .raw("carry", Obj::new().str("input", cin).str("output", cout).finish())
        .raw("axis", axis.map(|k| k.to_string()).unwrap_or_else(|| "null".into()))
        .raw("states", crate::json::strs(&states))
        .int("initial", init as i128)
        .raw("finals", arr(&finals.iter().map(|x| x.to_string()).collect::<Vec<_>>()))
        .raw("rows", arr(&rows_json))
        // The inputs a case keeps (§15.149), and the ones of them this table reads as a column.
        .raw("held_inputs", crate::json::strs(&m.held.iter().map(|x| x.text.clone()).collect::<Vec<_>>()))
        .raw("held", arr(&fixed_axes.iter().map(|k| k.to_string()).collect::<Vec<_>>()))
        .raw(
            "never",
            arr(&never_defs
                .iter()
                .map(|(a, b)| {
                    Obj::new()
                        .raw("states", arr(&a.iter().map(|x| x.to_string()).collect::<Vec<_>>()))
                        .raw("after", arr(&b.iter().map(|x| x.to_string()).collect::<Vec<_>>()))
                        .finish()
                })
                .collect::<Vec<_>>()),
        )
        .raw("once", arr(&once_defs))
        .raw("worlds", arr(&worlds_json))
        .raw("uncertified", crate::json::strs(&uncertified))
        .finish()
}

/// The `held` inputs of a machine whose value the certificate cannot follow through the
/// table that decides the transitions (§15.149): every one of them, when the table reads a
/// column that is not an input (a derived value, another table's answer, a count), and the
/// ones a `constraint` names. There one value of the input may allow calls another value of
/// the same coordinate does not, and a path of calls is not laid on the rows as one case's.
/// Drawn from nothing but the table's axes and the constraints, so that a re-checker draws the
/// same line from the certificate alone.
fn coupled_fixed(f: &RuleFile, m: &crate::ast::MachineDecl, ct: &CertTable) -> Vec<String> {
    let fixed: Vec<String> = m.held.iter().map(|n| n.text.clone()).collect();
    if ct.axes.iter().any(|a| a.kind != "input") {
        return fixed;
    }
    fixed.into_iter().filter(|x| f.constraints.iter().any(|k| &k.left == x || &k.right == x)).collect()
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
        CertCell::In(vs) | CertCell::NotIn(vs) => Obj::new()
            .str("cell", if matches!(c, CertCell::In(_)) { "in" } else { "not_in" })
            .raw(
                "values",
                arr(&vs.iter().map(|v| v.as_ref().map(|r| crate::json::quote(&rat(r))).unwrap_or_else(|| "null".into())).collect::<Vec<_>>()),
            )
            .finish(),
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
