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
                // A date stands for its day number, on the scale `ranges` gives date inputs, so
                // that a re-checker can compare it with them (§15.196).
                Lit::Date(y, m, d) => {
                    let v = crate::types::date_ord(*y, *m, *d);
                    o.str("type", "date").str("value", rat(&v)).finish()
                }
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

/// One `constraint` line: each side a name, or for a side that adds names up the list of them
/// (§15.216). A side of one name is written as the name, as every certificate before sums wrote
/// it, so a certificate of a rule with no sum is what it was.
fn constraint_json(left: &[String], op: &str, right: &[String]) -> String {
    let side = |names: &[String]| match names {
        [one] => crate::json::quote(one),
        many => crate::json::strs(many),
    };
    Obj::new().raw("left", side(left)).str("op", op).raw("right", side(right)).finish()
}

/// Every named value the rule computes, with the expression, the interval the declared
/// ranges force it into, and the integer that interval stores at its scale (§7.4, E108).
/// Re-checking one is interval arithmetic over the same expression — which is why the
/// ranges travel with it.
fn values_json(f: &RuleFile, c: &Checked) -> Vec<String> {
    // Every value `check` holds to E103 and E108, which is `derive`, `define` **and**
    // `result` — the last of these is neither `Item`, and leaving it out meant a rule whose
    // whole arithmetic is one `result` line stated no values at all (§15.99).
    // What each one is — a `derive`, a `define` or the `result` — travels with it, so that a
    // re-checker can tell for itself what kind of column an axis is (§15.196).
    let mut named: Vec<(&crate::ast::Name, &Expr, &str)> = Vec::new();
    for it in &f.items {
        match it {
            Item::Derived(d) => named.push((&d.name, &d.expr, "derive")),
            Item::Define(d) => named.push((&d.name, &d.expr, "define")),
            _ => {}
        }
    }
    let result_name;
    if let Some(r) = &f.result {
        result_name = crate::ast::Name { text: r.name.clone(), ascii: None, span: r.span.clone() };
        named.push((&result_name, &r.expr, "result"));
    }
    let mut out = Vec::new();
    for (name, e, of) in named {
        let Some(sym) = c.syms.get(&name.text) else { continue };
        let obj = Obj::new().str("name", &name.text).str("of", of).str("type", sym.ty.to_string()).raw("expr", expr_json(e, &sym.ty));
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
    // The days a date input takes from a koyomi file (§15.174): where they come from — the file
    // and its date as the rule names them, and the file's SHA-256 — and the days, on the scale
    // of `ranges`. Written only for a rule that has them, so no other certificate changes.
    let days = (!c.day_sets.is_empty()).then(|| {
        let mut d = Obj::new();
        let mut names: Vec<&String> = c.day_sets.keys().collect();
        names.sort();
        for n in names {
            let set = &c.day_sets[n];
            let from = match &set.from {
                Some(fr) => Obj::new().str("tool", "koyomi").str("file", &fr.file).str("date", &fr.date).finish(),
                None => "null".to_string(),
            };
            let list: Vec<String> = set.days.iter().map(|x| format!("\"{x}\"")).collect();
            d = d.raw(n, Obj::new().raw("from", from).str("sha256", &set.sha256).raw("days", format!("[{}]", list.join(","))).finish());
        }
        d.finish()
    });
    let o = Obj::new()
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
        // The scale every name is stored at (§7.1): its value is a whole number of `1/scale`.
        // A re-checker works out from these and from the values' expressions the step each
        // column's values sit on, and holds the step an axis is cut at to it (§15.190).
        .raw("scales", {
            let mut s = Obj::new();
            let mut names: Vec<&String> = c.syms.keys().filter(|n| c.scales.contains_key(*n)).collect();
            names.sort();
            for n in names {
                s = s.int(n, c.scales[n]);
            }
            s.finish()
        });
    let o = match days {
        Some(d) => o.raw("days", d),
        None => o,
    };
    o.raw("groups", {
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
                .map(|k| constraint_json(&k.left, k.op.word(), &k.right))
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
        // The enums whose values are ordered as `enums` lists them (§15.210): a cell `rank`
        // takes the values whose places pass its comparisons. Written only for a rule that has
        // one, so no other certificate changes.
        .opt_raw("ordered", (!c.ordered.is_empty()).then(|| {
            let mut names: Vec<&String> = c.ordered.iter().collect();
            names.sort();
            crate::json::strs(&names.into_iter().cloned().collect::<Vec<_>>())
        }))
        // The values an input of an ordered enum takes, where its `range` holds it to some
        // (§15.210): its axis runs over these and no others.
        .opt_raw("enum_ranges", (!c.enum_ranges.is_empty()).then(|| {
            let mut o = Obj::new();
            let mut names: Vec<&String> = c.enum_ranges.keys().collect();
            names.sort();
            for n in names {
                o = o.raw(n, crate::json::strs(&c.enum_ranges[n]));
            }
            o.finish()
        }))
        // The rule's inputs and the values its walks leave behind (`count`, `sum`), each by
        // name: with `values` and the tables' `decides`, what a re-checker works the kind of
        // every axis out from (§15.196). They are also the names whose ranges are declared
        // rather than computed.
        .raw("inputs", crate::json::strs(&f.inputs.iter().map(|i| i.name.text.clone()).collect::<Vec<_>>()))
        .raw(
            "walks",
            crate::json::strs(&f.items.iter().filter_map(|it| if let Item::Agg(d) = it { Some(d.name.text.clone()) } else { None }).collect::<Vec<_>>()),
        )
        .raw("values", arr(&values_json(f, c)))
        .raw("tables", arr(&tables))
        .raw("contracts", arr(&contracts))
        .raw("machine", machine_json(f, c, src))
        .finish()
}

/// The machine the rule is one step of, as ritsu's port of rules has it (`crate::ports`): its
/// states, where a case starts and ends, the inputs it holds, and the table that decides the next
/// state — its axes, and each row's coordinates, move and answers. The moves are read off the
/// rows as the certificate's `machine` reads them; where they cannot be, Err says why, in the
/// words the certificate gives. None for a rule that is no step of a machine.
pub fn machine_facts(f: &RuleFile, c: &Checked) -> Option<Result<ritsu_ports::Machine, String>> {
    use crate::ast::OutCell;
    let m = f.machine.as_ref()?;
    let (cin, cout) = m.carried()?;
    let Some(crate::types::Ty::Enum(en)) = c.ty_of(cin) else { return None };
    let states: Vec<String> = c.enums.get(&en).cloned().unwrap_or_default();
    let idx = |v: &str| states.iter().position(|x| x == v);
    let init = m.initial.as_ref()?.0.text.clone();
    let Some(set) = c.sets.iter().find(|s| s.table.outputs.iter().any(|o| o.name.text == cout)) else {
        return Some(Err(tr!("持ち越す出力を決める表がありません", "no table decides the carried output")));
    };
    if set.merged() {
        return Some(Err(tr!(
            "持ち越す出力を二つ以上の表が決めているので、行き先を一つの表の行から読めません",
            "several tables decide the carried output, so the transitions cannot be read off one table's rows"
        )));
    }
    let (Some(ct), Some(_)) = (certificate_of(set, c, f), crate::region::region_of(set, c, f)) else {
        return Some(Err(tr!("遷移を決める表に証明書がありません", "the table that decides the transitions has no certificate")));
    };
    let t = &set.table;
    let axis = ct.axes.iter().position(|a| a.name == cin);
    if let Some(k) = axis {
        if ct.axes[k].coords != states {
            return Some(Err(tr!("状態の軸が列挙の値の並びと違います", "the state's axis is not the enum's values in order")));
        }
    }
    let jout = t.outputs.iter().position(|o| o.name.text == cout).unwrap_or(0);
    let mut moves: std::collections::BTreeMap<usize, Option<usize>> = std::collections::BTreeMap::new();
    for r in &t.rows {
        let word = match r.outs.get(jout) {
            Some(OutCell::Lit(Lit::Word(w))) | Some(OutCell::Name(w)) => Some(w.as_str()),
            _ => None,
        };
        let mv = match word {
            Some(w) if w == cin => None,
            Some(w) if idx(w).is_some() => idx(w),
            _ => {
                return Some(Err(tr!(
                    "表 {} 行{} は次の状態を計算で決めているので、行から行き先を読めません",
                    "table {} row {} computes the next state, so its move cannot be read off the row",
                    t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                    r.index
                )))
            }
        };
        moves.insert(r.index, mv);
    }
    // where a case ends, in the order the enum declares the states (as `api` lists them)
    let finals = states.iter().enumerate().filter(|(_, v)| m.finals.iter().any(|x| &x.text == *v)).map(|(i, _)| i).collect();
    Some(Ok(ritsu_ports::Machine {
        name: m.name.text.clone(),
        carry_in: cin.to_string(),
        carry_out: cout.to_string(),
        state_enum: en.clone(),
        initial: idx(&init).unwrap_or(0),
        finals,
        held: m.held.iter().map(|x| x.text.clone()).collect(),
        policy: ct.policy.to_string(),
        axes: ct.axes.iter().map(|a| ritsu_ports::Axis { column: a.name.clone(), coords: a.coords.clone() }).collect(),
        state_axis: axis,
        decides: ct.decides.clone(),
        rows: ct.rows.iter().map(|r| ritsu_ports::MachineRow { row: r.row, accepts: r.accepts.clone(), to: moves.get(&r.row).copied().flatten(), produces: r.produces.clone() }).collect(),
        states,
    }))
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
                    // the values a comparison on an ordered enum covers (§15.210)
                    Cell::Ranked { values, .. } => values.clone(),
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

    // A machine that carries more than its state (§15.217): a call's way through the rows moves
    // its carried values too, so the paths of the state alone are not one case's, and the
    // claims over the values are laid in a section of their own below.
    let more_carried = !m.carries.is_empty();
    if more_carried && !finals.is_empty() {
        uncertified.push(tr!(
            "状態から終わりの状態へ行く呼び出しの並び（持ち越す値があるので、状態の行だけでは一つの案件の並びとして示せない）",
            "the sequences of calls to a final state (the machine carries more than its state, so the state's rows alone do not lay them out as one case's)"
        ));
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
        if more_carried {
            // Laid over the carried values, or stated: see the section below.
        } else if !finals.is_empty() && !coupled.is_empty() {
            if !finish_coupled_said {
                finish_coupled_said = true;
                uncertified.push(tr!(
                    "終わりの状態へ行く呼び出しの並び（遷移を決める表が入力でない値を読んでいるか、制約が {} を含んでいて、一つの値のまま通れるかを行から示せない）",
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
    // The values carried besides the state (§15.217), laid on the same rows.
    let carried = if more_carried {
        let state_to: Vec<Option<usize>> = moves.iter().map(|(_, mv, _)| match mv {
            Move::To(x) => Some(*x),
            Move::Stay => None,
        }).collect();
        let state_on: Vec<Vec<usize>> = moves.iter().map(|(_, _, on)| on.clone()).collect();
        let (json, more_uncertified) = carried_json(f, c, m, t, &ct, &state_to, &state_on, init, &finals, &never_defs, &once_counts, &fixed_axes, src);
        uncertified.extend(more_uncertified);
        Some(json)
    } else {
        None
    };
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
        .opt_raw("carried", carried)
        .raw("uncertified", crate::json::strs(&uncertified))
        .finish()
}

/// One coordinate of a table's axis as a closed interval of true values: the ends as written
/// for a point, and one grid step inside them for an interval, as a re-checker reads them.
fn closed_coord(ax: &crate::region::CertAxis, x: usize) -> Option<(Option<Rat>, Option<Rat>)> {
    let (lo, hi) = ax.bounds.get(x).cloned().flatten()?;
    if lo.is_some() && lo == hi {
        return Some((lo, hi));
    }
    match ax.step {
        Some(st) => Some((lo.map(|v| v.add(st)), hi.map(|v| v.sub(st)))),
        None => Some((lo, hi)),
    }
}

/// The axis of a table's certificate that is the machine's state, if the table reads it.
fn state_axis_of(ct: &CertTable, carried_in: &str) -> Option<usize> {
    ct.axes.iter().position(|a| a.name == carried_in)
}

/// What a carried value's next value can be, read off one row (§15.217).
#[derive(Clone)]
enum CarriedMove {
    Stay,
    /// A truth value or an enum's value the row writes: the coordinate it lies on.
    To(Option<usize>),
    /// A number the row writes.
    Lit(Rat),
    /// A value the rule computes: a `derive` or a `define` by name.
    Expr(String),
}

/// A value rounded down, and up, to a positive grid: where any of the five rounding modes can put it
/// (§7.2), as the re-checkers read an output's rounding.
fn floor_to(g: Rat, v: Rat) -> Rat {
    let q = v.div(g);
    g.mul(Rat::int(q.num.div_euclid(q.den)))
}

fn ceil_to(g: Rat, v: Rat) -> Rat {
    let q = v.div(g);
    g.mul(Rat::int(q.num.div_euclid(q.den) + i128::from(q.num.rem_euclid(q.den) != 0)))
}

/// The interval arithmetic a re-checker does over the certificate's expressions, here over the
/// rule's: the same shapes, the same answers, so that the set this side lays out is the set the
/// other side finds closed. A rounding goes out to its grid on both sides whatever its mode; a
/// name is looked up in `ranges` and nowhere else.
fn cert_interval(e: &Expr, ty: &crate::types::Ty, ranges: &std::collections::HashMap<String, (Rat, Rat)>) -> Option<(Rat, Rat)> {
    use std::cmp::Ordering;
    match e {
        Expr::Name(n, _) => ranges.get(n).copied(),
        Expr::Lit(Lit::Num(n), _) => {
            let v = crate::types::lit_value_in_pub(n, ty).or_else(|| crate::types::lit_value_in_pub(n, &crate::types::lit_ty_pub(n)))?;
            Some((v, v))
        }
        Expr::Lit(..) => None,
        Expr::Bin(l, op, r, _) => {
            let (al, ah) = cert_interval(l, ty, ranges)?;
            let (bl, bh) = cert_interval(r, ty, ranges)?;
            match op {
                BinOp::Add => Some((al.checked_add(bl)?, ah.checked_add(bh)?)),
                BinOp::Sub => Some((al.checked_sub(bh)?, ah.checked_sub(bl)?)),
                BinOp::Mul | BinOp::Div => {
                    if *op == BinOp::Div && bl.cmp_to(Rat::zero()) != Ordering::Greater && bh.cmp_to(Rat::zero()) != Ordering::Less {
                        return None;
                    }
                    let g = |x: Rat, y: Rat| if *op == BinOp::Mul { x.checked_mul(y) } else { x.checked_div(y) };
                    let mut vs = [g(al, bl)?, g(al, bh)?, g(ah, bl)?, g(ah, bh)?];
                    vs.sort_by(|a, b| a.cmp_to(*b));
                    Some((vs[0], vs[3]))
                }
                _ => None,
            }
        }
        Expr::Call(name, args, _) => {
            let (lo, hi) = cert_interval(args.first()?, ty, ranges)?;
            match (name.as_str(), args.len()) {
                (crate::kw::MIN, 2) | (crate::kw::MAX, 2) => {
                    let (bl, bh) = cert_interval(&args[1], ty, ranges)?;
                    let pick = |a: Rat, b: Rat| {
                        let less = a.cmp_to(b) == Ordering::Less;
                        if (name == crate::kw::MIN) == less { a } else { b }
                    };
                    Some((pick(lo, bl), pick(hi, bh)))
                }
                (m, 2) if crate::num::RoundMode::parse(m).is_some() => {
                    let (g, g2) = cert_interval(&args[1], ty, ranges)?;
                    if g != g2 || g.num == 0 {
                        return None;
                    }
                    let g = if g.num < 0 { Rat::zero().sub(g) } else { g };
                    let floor = |v: Rat| {
                        let q = v.div(g);
                        g.mul(Rat::int(q.num.div_euclid(q.den)))
                    };
                    let ceil = |v: Rat| {
                        let q = v.div(g);
                        g.mul(Rat::int(q.num.div_euclid(q.den) + i128::from(q.num.rem_euclid(q.den) != 0)))
                    };
                    Some((floor(lo), ceil(hi)))
                }
                _ => None,
            }
        }
    }
}

/// The section a machine that carries more than its state gets (§15.217): its carried values
/// laid on the rows of the table that decides the transitions, as the state is.
///
/// A place a case can be is the state together with one coordinate of each carried value:
/// every value of a number when the machine was walked whole, the intervals the walk cut it
/// into otherwise, the two truth values, an enum's values. A row takes a place when its box
/// takes the state and, on each column the table reads a carried value in, meets that value's
/// coordinate; it moves the state as it does above, and each carried value to what its cell
/// writes — the value, the same value, or the interval the named computation comes to over the
/// place, by the arithmetic a re-checker does — rounded out to the grid its output rounds to
/// (§15.232), on every coordinate that interval meets, and outside the range where it reaches
/// past it. That is more than the calls can do, so a set
/// that holds where a case starts and is closed under it holds every place a case reaches, and
/// the claims that nothing bad is reached are closures of that kind, as above. What the rows
/// cannot carry is listed as not certified, as above; nothing here says a case can still finish
/// — that needs calls that really happen.
#[allow(clippy::too_many_arguments)]
fn carried_json(
    f: &RuleFile,
    c: &Checked,
    m: &crate::ast::MachineDecl,
    t: &crate::ast::Table,
    ct: &CertTable,
    state_to: &[Option<usize>],
    state_on: &[Vec<usize>],
    init: usize,
    finals: &[usize],
    never_defs: &[(Vec<usize>, Vec<usize>)],
    once_counts: &[Option<Vec<(usize, bool)>>],
    fixed_axes: &[usize],
    src: &str,
) -> (String, Vec<String>) {
    use crate::ast::{Cell, OutCell};
    use std::cmp::Ordering;
    use std::collections::{HashMap, HashSet, VecDeque};
    let not_laid = |why: String| -> (String, Vec<String>) {
        let note = tr!("持ち越す値についての主張（{why}）", "the claims about the carried values ({why})");
        (Obj::new().str("why", &why).finish(), vec![note])
    };
    if !fixed_axes.is_empty() {
        return not_laid(tr!("遷移を決める表が `held` の入力を列として読んでいる", "the table that decides the transitions reads a `held` input as a column"));
    }
    let Some(a) = crate::machine::analyze(f, c, crate::region::DEFAULT_BUDGET as usize) else {
        return not_laid(tr!("持ち越す値を読めない", "the carried values cannot be read"));
    };
    if a.blocked.is_some() || a.over_budget || a.more.is_empty() {
        return not_laid(tr!("入力の区画を調べきれていない", "the inputs' cells were not all walked"));
    }
    // Each carried value: its coordinates as closed intervals (a truth value 0 and 1, an enum's
    // values by their place), and what each row writes into it.
    let mut items: Vec<String> = Vec::new();
    let mut coords: Vec<Vec<(Rat, Rat)>> = Vec::new();
    let mut labels: Vec<Option<Vec<String>>> = Vec::new();
    let mut table_axis: Vec<Option<usize>> = Vec::new();
    let mut moves: Vec<Vec<CarriedMove>> = Vec::new();
    let mut grids: Vec<Option<Rat>> = Vec::new();
    for it in &a.more {
        let Some(jo) = t.outputs.iter().position(|o| o.name.text == it.output) else {
            return not_laid(tr!("{} はこの表の外で決まる", "{} is decided outside this table", it.output));
        };
        let (cs, ls): (Vec<(Rat, Rat)>, Option<Vec<String>>) = match &it.ty {
            crate::types::Ty::Bool => (vec![(Rat::int(0), Rat::int(0)), (Rat::int(1), Rat::int(1))], Some(vec![crate::kw::FALSE.into(), crate::kw::TRUE.into()])),
            crate::types::Ty::Enum(_) => {
                let words: Vec<String> = it.axis.coords.iter().map(|co| if let crate::vdiff::Coord::Word(w) = co { w.clone() } else { String::new() }).collect();
                ((0..words.len()).map(|i| (Rat::int(i as i128), Rat::int(i as i128))).collect(), Some(words))
            }
            _ => {
                let mut v = Vec::new();
                for co in &it.axis.coords {
                    let (Some(lo), Some(hi)) = crate::vdiff::ival(co, it.axis.step) else {
                        return not_laid(tr!("{} の座標に端の無いものがある", "a coordinate of {} has an open end", it.input));
                    };
                    v.push((lo, hi));
                }
                (v, None)
            }
        };
        let word_at = |w: &str| ls.as_ref().and_then(|l| l.iter().position(|x| x == w));
        let mut mv: Vec<CarriedMove> = Vec::new();
        let mut moves_json: Vec<String> = Vec::new();
        for r in &t.rows {
            let (m1, o) = match r.outs.get(jo) {
                Some(OutCell::Name(n)) if *n == it.input => (CarriedMove::Stay, Obj::new().int("row", r.index as i128).bool("stay", true)),
                Some(OutCell::Name(w)) if ls.is_some() && word_at(w).is_some() => {
                    let to = word_at(w);
                    (CarriedMove::To(to), Obj::new().int("row", r.index as i128).int("to", to.unwrap_or(0) as i128))
                }
                Some(OutCell::Lit(Lit::Num(n))) if ls.is_none() => {
                    let Some(v) = crate::types::lit_value_in_pub(n, &it.ty) else {
                        return not_laid(tr!(
                            "表 {} 行{} が {} に書く値を読めない",
                            "what table {} row {} writes into {} cannot be read",
                            t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                            r.index,
                            it.output
                        ));
                    };
                    let to = cs.iter().position(|(lo, hi)| lo.cmp_to(v) != Ordering::Greater && hi.cmp_to(v) != Ordering::Less);
                    match to {
                        Some(x) => (CarriedMove::Lit(v), Obj::new().int("row", r.index as i128).int("to", x as i128)),
                        None => (CarriedMove::Lit(v), Obj::new().int("row", r.index as i128).bool("out", true)),
                    }
                }
                Some(OutCell::Name(n))
                    if ls.is_none()
                        && f.items.iter().any(|x| matches!(x, Item::Derived(d) if d.name.text == *n) || matches!(x, Item::Define(d) if d.name.text == *n)) =>
                {
                    (CarriedMove::Expr(n.clone()), Obj::new().int("row", r.index as i128).str("expr", n))
                }
                _ => {
                    return not_laid(tr!(
                        "表 {} 行{} が {} に書く値を、行から読めない",
                        "what table {} row {} writes into {} cannot be read off the row",
                        t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                        r.index,
                        it.output
                    ))
                }
            };
            let o = match r.out_spans.get(jo) {
                Some(sp) => o.raw(
                    "source",
                    Obj::new().int("line", sp.line as i128).int("col", sp.col as i128).int("len", sp.len as i128).str("text", &at_span(src, sp.line, sp.col, sp.len)).finish(),
                ),
                None => o,
            };
            mv.push(m1);
            moves_json.push(o.finish());
        }
        let ak = ct.axes.iter().position(|x| x.name == it.input);
        let kind = match &it.ty {
            crate::types::Ty::Bool => "bool",
            crate::types::Ty::Enum(_) => "enum",
            _ => "number",
        };
        let mut o = Obj::new()
            .str("input", &it.input)
            .str("output", &it.output)
            .str("kind", kind)
            .raw("axis", ak.map(|x| x.to_string()).unwrap_or_else(|| "null".into()))
            .raw("coords", arr(&cs.iter().map(|(lo, hi)| format!("[{},{}]", crate::json::quote(&rat(lo)), crate::json::quote(&rat(hi)))).collect::<Vec<_>>()));
        if let Some(l) = &ls {
            o = o.raw("labels", crate::json::strs(l));
        }
        if let Some((lo, hi)) = it.range {
            o = o.raw("range", format!("[{},{}]", crate::json::quote(&rat(&lo)), crate::json::quote(&rat(&hi)))).raw("step", crate::json::quote(&rat(&it.axis.step)));
        }
        // The grid the output rounds to (§7.2: every numeric output declares one, E104): what the call
        // passes on is the rounding of what the row writes, which a re-checker lands on the
        // coordinates (§15.232).
        let round = if ls.is_none() {
            f.outputs
                .iter()
                .find(|o| o.name.text == it.output)
                .and_then(|od| od.rounding.as_ref())
                .and_then(|rd| crate::types::lit_value_in_pub(&rd.grid, &it.ty))
                .map(|g| if g.num < 0 { Rat::zero().sub(g) } else { g })
                .filter(|g| g.num != 0)
        } else {
            None
        };
        if let Some(g) = round {
            o = o.raw("round", crate::json::quote(&rat(&g)));
        }
        items.push(o.raw("moves", arr(&moves_json)).finish());
        grids.push(round);
        coords.push(cs);
        labels.push(ls);
        table_axis.push(ak);
        moves.push(mv);
    }
    let out = usize::MAX;
    // Whether a row's box meets a place on a carried value's column.
    let meets = |ri: usize, k: usize, ci: usize| -> bool {
        let Some(ak) = table_axis[k] else { return true };
        let Some(cr) = ct.rows.get(ri) else { return false };
        let Some(acc) = cr.accepts.get(ak) else { return false };
        let ax = &ct.axes[ak];
        acc.iter().any(|&x| match &labels[k] {
            Some(l) => ax.coords.get(x) == l.get(ci),
            None => match closed_coord(ax, x) {
                Some((blo, bhi)) => {
                    let (lo, hi) = coords[k][ci];
                    blo.is_none_or(|b| b.cmp_to(hi) != Ordering::Greater) && bhi.is_none_or(|b| b.cmp_to(lo) != Ordering::Less)
                }
                None => true,
            },
        })
    };
    // The inputs' declared ranges, the ground every interval stands on.
    let mut base: HashMap<String, (Rat, Rat)> = HashMap::new();
    for i in &f.inputs {
        if let Some((Some(lo), Some(hi))) = c.ranges.get(&i.name.text) {
            base.insert(i.name.text.clone(), (*lo, *hi));
        }
    }
    let values: Vec<(String, Expr, crate::types::Ty)> = f
        .items
        .iter()
        .filter_map(|x| match x {
            Item::Derived(d) => Some((d.name.text.clone(), d.expr.clone(), c.ty_of(&d.name.text).unwrap_or(crate::types::Ty::Unknown))),
            Item::Define(d) => Some((d.name.text.clone(), d.expr.clone(), c.ty_of(&d.name.text).unwrap_or(crate::types::Ty::Unknown))),
            _ => None,
        })
        .collect();
    // What a computed value comes to at a place, through a row: its expression over the
    // inputs' ranges with each carried number at the place's interval, narrowed by the row's box
    // where the table reads the value as a column.
    let image = |ri: usize, cc: &[usize], name: &str| -> Option<(Rat, Rat)> {
        let mut ranges = base.clone();
        for (k, it) in a.more.iter().enumerate() {
            if labels[k].is_none() {
                ranges.insert(it.input.clone(), coords[k][cc[k]]);
            }
        }
        for _ in 0..=values.len() {
            let mut moved = false;
            for (n, e, ty) in &values {
                if ranges.contains_key(n) {
                    continue;
                }
                if let Some(iv) = cert_interval(e, ty, &ranges) {
                    ranges.insert(n.clone(), iv);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        let (mut lo, mut hi) = *ranges.get(name)?;
        if let Some(ak) = ct.axes.iter().position(|x| x.name == name) {
            let acc = ct.rows.get(ri)?.accepts.get(ak)?;
            let bs: Vec<(Option<Rat>, Option<Rat>)> = acc.iter().map(|&x| closed_coord(&ct.axes[ak], x).unwrap_or((None, None))).collect();
            let hull_lo = bs.iter().map(|b| b.0).try_fold(None::<Rat>, |m, b| b.map(|b| Some(m.map_or(b, |m| if b.cmp_to(m) == Ordering::Less { b } else { m })))).flatten();
            let hull_hi = bs.iter().map(|b| b.1).try_fold(None::<Rat>, |m, b| b.map(|b| Some(m.map_or(b, |m| if b.cmp_to(m) == Ordering::Greater { b } else { m })))).flatten();
            if let Some(h) = hull_lo {
                if h.cmp_to(lo) == Ordering::Greater {
                    lo = h;
                }
            }
            if let Some(h) = hull_hi {
                if h.cmp_to(hi) == Ordering::Less {
                    hi = h;
                }
            }
        }
        Some((lo, hi))
    };
    type Place = (usize, Vec<usize>);
    // Under `policy first`, a row an earlier one covers at a place does not answer there: the
    // earlier row takes the place's state and the whole of each carried value's coordinate on
    // the columns the table reads them in, and on every other column all that the later row
    // takes. One earlier row has to cover it alone; two that cover it between them are not
    // looked for, which leaves the later row in — more calls than happen, never fewer.
    let first = ct.policy == "first";
    let covers = |r0: usize, ri: usize, s: usize, cc: &[usize]| -> bool {
        let (Some(c0), Some(c1)) = (ct.rows.get(r0), ct.rows.get(ri)) else { return false };
        if !state_on[r0].contains(&s) {
            return false;
        }
        for (ai, ax) in ct.axes.iter().enumerate() {
            let (Some(a0), Some(a1)) = (c0.accepts.get(ai), c1.accepts.get(ai)) else { return false };
            if Some(ai) == state_axis_of(ct, &a.carry.0) {
                continue;
            }
            match (0..cc.len()).find(|&k| table_axis[k] == Some(ai)) {
                Some(k) => {
                    let whole = (0..ax.coords.len()).filter(|&x| match &labels[k] {
                        Some(l) => ax.coords.get(x) == l.get(cc[k]),
                        None => match closed_coord(ax, x) {
                            Some((blo, bhi)) => {
                                let (lo, hi) = coords[k][cc[k]];
                                blo.is_none_or(|b| b.cmp_to(hi) != Ordering::Greater) && bhi.is_none_or(|b| b.cmp_to(lo) != Ordering::Less)
                            }
                            None => true,
                        },
                    });
                    if !whole.into_iter().all(|x| a0.contains(&x)) {
                        return false;
                    }
                }
                None => {
                    if !a1.iter().all(|x| a0.contains(x)) {
                        return false;
                    }
                }
            }
        }
        true
    };
    // Where a carried number lands: what the row writes — its own coordinate, a value written in the
    // row, a computed value's interval over the place — rounded out to the grid the output rounds
    // to, since the rounded value is what the next call is passed (§15.232); every coordinate that
    // meets it, and outside the range where it reaches past.
    let land = |k: usize, (lo, hi): (Rat, Rat)| -> Vec<usize> {
        let (lo, hi) = match grids[k] {
            Some(g) => (floor_to(g, lo), ceil_to(g, hi)),
            None => (lo, hi),
        };
        let (rlo, rhi) = (coords[k][0].0, coords[k][coords[k].len() - 1].1);
        let mut l: Vec<usize> = (0..coords[k].len())
            .filter(|&ci| coords[k][ci].0.cmp_to(hi) != Ordering::Greater && coords[k][ci].1.cmp_to(lo) != Ordering::Less)
            .collect();
        if lo.cmp_to(rlo) == Ordering::Less || hi.cmp_to(rhi) == Ordering::Greater {
            l.push(out);
        }
        l
    };
    // One call from a place, through every row that takes it: (row number, where it goes).
    let step = |p: &Place| -> Option<Vec<(usize, Place)>> {
        let (s, cc) = p;
        if cc.contains(&out) {
            return Some(Vec::new());
        }
        let mut next = Vec::new();
        for (ri, r) in t.rows.iter().enumerate() {
            if !state_on[ri].contains(s) || !(0..cc.len()).all(|k| meets(ri, k, cc[k])) {
                continue;
            }
            if first && (0..ri).any(|r0| covers(r0, ri, *s, cc)) {
                continue;
            }
            let s2 = state_to[ri].unwrap_or(*s);
            let mut lists: Vec<Vec<usize>> = Vec::new();
            for k in 0..cc.len() {
                lists.push(match &moves[k][ri] {
                    CarriedMove::To(Some(x)) => vec![*x],
                    CarriedMove::To(None) => vec![out],
                    CarriedMove::Stay if labels[k].is_some() => vec![cc[k]],
                    CarriedMove::Stay => land(k, coords[k][cc[k]]),
                    CarriedMove::Lit(v) => land(k, (*v, *v)),
                    CarriedMove::Expr(n) => land(k, image(ri, cc, n)?),
                });
            }
            let mut combos: Vec<Vec<usize>> = vec![Vec::new()];
            for l in &lists {
                combos = combos.into_iter().flat_map(|base| l.iter().map(move |x| [base.clone(), vec![*x]].concat())).collect();
            }
            for cc2 in combos {
                next.push((r.index, (s2, cc2)));
            }
        }
        Some(next)
    };
    let start: Place = (init, a.more.iter().map(|it| it.start).collect());
    // The places a case can be, closed under a call; a bound on how many, as the walk has.
    let mut seen: Vec<Place> = vec![start.clone()];
    let mut seen_set: HashSet<Place> = seen.iter().cloned().collect();
    let mut q: VecDeque<Place> = VecDeque::from([start.clone()]);
    let mut succ: HashMap<Place, Vec<(usize, Place)>> = HashMap::new();
    while let Some(p) = q.pop_front() {
        let Some(ns) = step(&p) else {
            return not_laid(tr!("計算した値が持ち越す値になる行で、その値の範囲を区間の計算で出せない", "a row carries on a computed value whose interval the arithmetic does not bound"));
        };
        for (_, n2) in &ns {
            if seen_set.insert(n2.clone()) {
                seen.push(n2.clone());
                q.push_back(n2.clone());
            }
        }
        succ.insert(p, ns);
        if seen.len() > 200_000 {
            return not_laid(tr!("持ち越す値の組が多すぎる", "there are too many places a case can be"));
        }
    }
    let mut uncertified: Vec<String> = Vec::new();
    let place_json = |p: &Place, extra: Option<String>| -> String {
        let cc: Vec<String> = p.1.iter().map(|&x| if x == out { "-1".to_string() } else { x.to_string() }).collect();
        match extra {
            Some(e) => format!("[{},[{}],{e}]", p.0, cc.join(",")),
            None => format!("[{},[{}]]", p.0, cc.join(",")),
        }
    };
    // `final`: from a place in a final state, every call stays in that state.
    let final_ok = seen.iter().filter(|p| finals.contains(&p.0)).all(|p| succ.get(p).is_none_or(|ns| ns.iter().all(|(_, n2)| n2.0 == p.0)));
    if !final_ok && seen.iter().any(|p| finals.contains(&p.0)) {
        uncertified.push(tr!("持ち越す値の上で、終わりの状態から出る呼び出しが無いこと", "that no call leaves a final state, over the carried values"));
    }
    // `never A after B`.
    let mut nevers: Vec<String> = Vec::new();
    for (li, (aa, bb)) in never_defs.iter().enumerate() {
        let first = (start.clone(), bb.contains(&init));
        let mut set: Vec<(Place, bool)> = vec![first.clone()];
        let mut have: HashSet<(Place, bool)> = set.iter().cloned().collect();
        let mut q2: VecDeque<(Place, bool)> = VecDeque::from([first]);
        while let Some((p, sn)) = q2.pop_front() {
            for (_, n2) in succ.get(&p).cloned().unwrap_or_default() {
                let x = (n2.clone(), sn || bb.contains(&n2.0));
                if have.insert(x.clone()) {
                    set.push(x.clone());
                    q2.push_back(x);
                }
            }
        }
        if set.iter().any(|(p, sn)| *sn && aa.contains(&p.0)) {
            uncertified.push(tr!("持ち越す値の上で、`never` の行（{} 本目）", "the `never` line (number {}), over the carried values", li + 1));
            nevers.push("null".into());
        } else {
            nevers.push(arr(&set.iter().map(|(p, sn)| place_json(p, Some(sn.to_string()))).collect::<Vec<_>>()));
        }
    }
    // `once`.
    let mut onces: Vec<String> = Vec::new();
    for (li, fl) in once_counts.iter().enumerate() {
        let Some(flags) = fl else {
            onces.push("null".into());
            continue;
        };
        let counts = |row: usize| flags.iter().any(|(x, y)| *x == row && *y);
        let first = (start.clone(), 0u8);
        let mut set: Vec<(Place, u8)> = vec![first.clone()];
        let mut have: HashSet<(Place, u8)> = set.iter().cloned().collect();
        let mut q2: VecDeque<(Place, u8)> = VecDeque::from([first]);
        while let Some((p, n)) = q2.pop_front() {
            for (row, n2) in succ.get(&p).cloned().unwrap_or_default() {
                let x = (n2.clone(), (n + u8::from(counts(row))).min(2));
                if have.insert(x.clone()) {
                    set.push(x.clone());
                    q2.push_back(x);
                }
            }
        }
        if set.iter().any(|(_, n)| *n >= 2) {
            uncertified.push(tr!("持ち越す値の上で、`once` の行（{} 本目）", "the `once` line (number {}), over the carried values", li + 1));
            onces.push("null".into());
        } else {
            onces.push(arr(&set.iter().map(|(p, n)| place_json(p, Some(n.to_string()))).collect::<Vec<_>>()));
        }
    }
    // `always`: each test held to its value's coordinate at every place in the states it names.
    let state_idx = |w: &str| c.enums.get(&a.state_enum).and_then(|vs| vs.iter().position(|x| x == w));
    let mut alwayses: Vec<String> = Vec::new();
    for (li, al) in m.alwayses.iter().enumerate() {
        let when: Vec<usize> = al.when.iter().filter_map(|w| state_idx(&w.text)).collect();
        let mut tests: Vec<String> = Vec::new();
        let mut fits: Vec<(usize, Box<dyn Fn(usize) -> bool>)> = Vec::new();
        for (n, cell, _) in &al.tests {
            let Some(k) = a.more.iter().position(|it| it.input == n.text) else { continue };
            let ty = a.more[k].ty.clone();
            let lit = |l: &Lit| -> Option<Rat> {
                match l {
                    Lit::Num(x) => crate::types::lit_value_in_pub(x, &ty),
                    _ => None,
                }
            };
            let word = |l: &Lit| -> Option<String> {
                match l {
                    Lit::Word(w) => Some(w.clone()),
                    _ => None,
                }
            };
            let cert_cell = match cell {
                Cell::Cmp(ops) => CertCell::Cmp(ops.iter().map(|(op, l)| (op.word(), lit(l))).collect()),
                Cell::Lit(l) if labels[k].is_some() => CertCell::Is(word(l).into_iter().collect()),
                Cell::Set(ls) if labels[k].is_some() => CertCell::Is(ls.iter().filter_map(word).collect()),
                Cell::Not(ls) if labels[k].is_some() => CertCell::Not(ls.iter().filter_map(word).collect()),
                Cell::Lit(l) => CertCell::In(vec![lit(l)]),
                Cell::Set(ls) => CertCell::In(ls.iter().map(lit).collect()),
                Cell::Not(ls) => CertCell::NotIn(ls.iter().map(lit).collect()),
                _ => CertCell::Any,
            };
            tests.push(Obj::new().int("item", k as i128).raw("cell", cell_json(&cert_cell)).finish());
            let (cs, ls) = (coords[k].clone(), labels[k].clone());
            let f2: Box<dyn Fn(usize) -> bool> = Box::new(move |ci: usize| -> bool {
                if ci == usize::MAX {
                    return false;
                }
                let (lo, hi) = cs[ci];
                match (&cert_cell, &ls) {
                    (CertCell::Is(ws), Some(l)) => ws.contains(&l[ci]),
                    (CertCell::Not(ws), Some(l)) => !ws.contains(&l[ci]),
                    (CertCell::Cmp(ops), None) => ops.iter().all(|(op, v)| {
                        let Some(v) = v else { return false };
                        [lo, hi].iter().all(|x| match *op {
                            "<=" => x.cmp_to(*v) != Ordering::Greater,
                            "<" => x.cmp_to(*v) == Ordering::Less,
                            ">=" => x.cmp_to(*v) != Ordering::Less,
                            _ => x.cmp_to(*v) == Ordering::Greater,
                        })
                    }),
                    (CertCell::In(vs), None) => lo == hi && vs.iter().any(|v| *v == Some(lo)),
                    (CertCell::NotIn(vs), None) => vs.iter().all(|v| v.is_some_and(|v| v.cmp_to(lo) == Ordering::Less || v.cmp_to(hi) == Ordering::Greater)),
                    (CertCell::Any, _) => true,
                    _ => false,
                }
            });
            fits.push((k, f2));
        }
        // A place outside a range is the range's claim, not this one's (E134).
        let holds = seen
            .iter()
            .filter(|p| (when.is_empty() || when.contains(&p.0)) && !p.1.contains(&usize::MAX))
            .all(|p| fits.iter().all(|(k, f2)| f2(p.1[*k])));
        if !holds {
            uncertified.push(tr!("持ち越す値の上で、`always` の行（{} 本目）", "the `always` line (number {}), over the carried values", li + 1));
        }
        alwayses.push(
            Obj::new()
                .raw("when", arr(&when.iter().map(|x| x.to_string()).collect::<Vec<_>>()))
                .raw("tests", arr(&tests))
                .bool("holds", holds)
                .finish(),
        );
    }
    // A carried number stays within its input's range.
    let mut ranges_json: Vec<String> = Vec::new();
    for (k, it) in a.more.iter().enumerate() {
        if labels[k].is_some() {
            continue;
        }
        let holds = !seen.iter().any(|p| p.1[k] == out);
        if !holds {
            uncertified.push(tr!("持ち越す値の上で、{} が範囲に収まること", "that {} stays within its range, over the carried values", it.output));
        }
        ranges_json.push(Obj::new().int("item", k as i128).bool("holds", holds).finish());
    }
    let json = Obj::new()
        .raw("items", arr(&items))
        .raw("start", format!("[{}]", start.1.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")))
        .raw("places", arr(&seen.iter().map(|p| place_json(p, None)).collect::<Vec<_>>()))
        .bool("final_certified", final_ok)
        .raw("never", arr(&nevers))
        .raw("once", arr(&onces))
        .raw("always", arr(&alwayses))
        .raw("ranges", arr(&ranges_json))
        .finish();
    (json, uncertified)
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
    fixed.into_iter().filter(|x| f.constraints.iter().any(|k| k.names().any(|n| n == x))).collect()
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
    // What the rows write into the columns that hold a number, for a table that has one: written
    // only there, so no other certificate changes (§15.196).
    let writes_numbers = t.rows.iter().any(|r| r.writes.iter().any(|w| w.is_some()));
    let rows: Vec<String> = t
        .rows
        .iter()
        .map(|r| {
            let accepts: Vec<String> = r.accepts.iter().map(|xs| arr(&xs.iter().map(|x| x.to_string()).collect::<Vec<_>>())).collect();
            let writes = arr(&r
                .writes
                .iter()
                .enumerate()
                .map(|(oi, w)| match w {
                    Some((e, ty, at)) => {
                        let o = Obj::new().raw("expr", expr_json(e, ty)).raw(
                            "source",
                            match at {
                                Some((line, col, len)) => Obj::new()
                                    .int("line", *line as i128)
                                    .int("col", *col as i128)
                                    .int("len", *len as i128)
                                    .str("text", at_span(src, *line, *col, *len))
                                    .finish(),
                                None => "null".into(),
                            },
                        );
                        // The values the row's own cell lets the name it writes take, where that
                        // narrows the column's range (§15.213). Only there, so no other
                        // certificate changes.
                        match r.inside.get(oi).copied().flatten() {
                            Some((lo, hi)) => o.raw("inside", format!("[{},{}]", end(&Some(lo)), end(&Some(hi)))).finish(),
                            None => o.finish(),
                        }
                    }
                    None => "null".into(),
                })
                .collect::<Vec<_>>());
            let o = Obj::new()
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
                );
            if writes_numbers { o.raw("writes", writes).finish() } else { o.finish() }
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
    let o = Obj::new()
        .str("table", &t.name)
        .str("policy", t.policy)
        .int("outputs", t.outputs as i128)
        .raw("axes", arr(&axes))
        .raw("decides", crate::json::strs(&t.decides))
        .raw("rows", arr(&rows))
        .raw("disjoint", arr(&disjoint))
        .raw("refuted", arr(&refuted));
    // The pairs koyomi's days part (§15.174), written only for a table that has them, so no
    // other certificate changes.
    let o = if t.days_apart.is_empty() {
        o
    } else {
        o.raw("days_apart", arr(&t.days_apart.iter().map(|(a, b, ai)| Obj::new().int("a", *a as i128).int("b", *b as i128).int("axis", *ai as i128).finish()).collect::<Vec<_>>()))
    };
    // The pairs the rows of a table above part (§15.196): the axis, and for each value both rows
    // take there, the rows that write it with why each fires nowhere in what both take. Written
    // only for a table that has them.
    let o = if t.above_apart.is_empty() {
        o
    } else {
        o.raw(
            "above_apart",
            arr(&t
                .above_apart
                .iter()
                .map(|(a, b, ai, at)| {
                    Obj::new()
                        .int("a", *a as i128)
                        .int("b", *b as i128)
                        .int("axis", *ai as i128)
                        .raw("at", arr(&at.iter().map(|x| Obj::new().int("coord", x.coord as i128).raw("above_rows", above_rows_json(x, &t.model)).finish()).collect::<Vec<_>>()))
                        .finish()
                })
                .collect::<Vec<_>>()),
        )
    };
    o
        .raw("undecided", arr(&undecided))
        .raw("reach", arr(&reach))
        .raw("unused", arr(&t.unused.iter().map(|r| r.to_string()).collect::<Vec<_>>()))
        .raw("unreachable", arr(&t.unreachable.iter().map(|r| r.to_string()).collect::<Vec<_>>()))
        .raw(
            "constraints",
            arr(&t
                .constraints
                .iter()
                .map(|(l, op, r)| constraint_json(l, op, r))
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
/// `derive`'s equation is two facts, `name − expr <= 0` (`le`) and `>= 0`, and so is the
/// equation of a `define` of a number whose expression is linear (§15.209); a range is two,
/// its low end and its high end; a `constraint` is one, by its place in `constraints`.
fn fact_json(o: &crate::fourier::Origin) -> String {
    use crate::fourier::Origin;
    match o {
        Origin::Derive { name, le } => Obj::new().str("derive", name).bool("le", *le).finish(),
        // A `define` of a number whose expression is linear: the same equation, built again from
        // the define's expression in `values` (§15.209).
        Origin::Define { name, le } => Obj::new().str("define", name).bool("le", *le).finish(),
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
                // One end of what a row of a table above takes on a numeric column of its own,
                // by the column's name (§15.195): a re-checker holds every coordinate the row
                // takes there to it, in that table's own part of the certificate.
                Origin::Above { name, hi } => {
                    let at = if *hi { q.k.mul(Rat::int(-1)) } else { q.k };
                    Obj::new().raw("cond", Obj::new().str("name", name).bool("hi", *hi).str("at", rat(&at)).bool("open", q.strict).finish())
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
        Cover::ByDays(ai) => Obj::new().int("days_axis", *ai as i128).finish(),
        Cover::ByDefine(ai) => Obj::new().int("define_axis", *ai as i128).finish(),
        Cover::ByTruth(ai) => Obj::new().int("truth_axis", *ai as i128).finish(),
        // The column a table above decides, the value the box takes, and every row of that
        // table that writes the value, by its number there, each with why it fires nowhere in
        // the box: an axis of this table on which the box takes none of the words the row lets
        // in, or the multipliers of a refutation (§15.195).
        Cover::ByAboveRows(a) => Obj::new().raw("above_rows", above_rows_json(a, model)).finish(),
    }
}

/// The rows of a table above that write the value a box holds on the column it decides, each with
/// why it fires nowhere in the box: a cover's leaf, and each value of a pair the rows part
/// (§15.195, §15.196).
fn above_rows_json(a: &crate::region::AboveRows, model: &[crate::fourier::Origin]) -> String {
    Obj::new()
        .int("axis", a.axis as i128)
        .str("column", &a.column)
        .str("value", &a.value)
        .raw(
            "rows",
            arr(&a
                .rows
                .iter()
                .map(|(ri, o)| match o {
                    crate::region::RowOut::Clash(aj) => Obj::new().int("row", *ri as i128 + 1).int("clash", *aj as i128).finish(),
                    crate::region::RowOut::Farkas(r) => Obj::new().int("row", *ri as i128 + 1).raw("farkas", farkas_json(r, model)).finish(),
                })
                .collect::<Vec<_>>()),
        )
        .finish()
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
        // The comparisons as written, against values of the enum by name; the re-checker reads
        // the order from `ordered` (§15.210).
        CertCell::Rank(cs) => Obj::new()
            .str("cell", "rank")
            .raw("tests", arr(&cs.iter().map(|(op, v)| Obj::new().str("op", op).str("value", v).finish()).collect::<Vec<_>>()))
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
