//! The effects of a module's functions (DESIGN 4): which kinds of failure each can end in, and
//! whether it may not stop.
//!
//! - A function's own kinds are those of the places in its body that are not shown (`ranges.rs`):
//!   an example, an undecided place, and a place that waits for the checks across the languages
//!   (ritsu-cross's X2, X3, X4, X6), which is counted as undecided until its answer comes.
//! - A function takes the effect of each function it calls, and the functions of a ring of calls
//!   (a strongly connected component of the calls, or a function that calls itself) share the kinds
//!   of all of them and may not stop (DESIGN 4.3). A call the analysis read the called function's
//!   body again for (a generic function, DESIGN 26.5) takes what that reading found instead: the
//!   kinds of its places that are not shown, and the effects of the calls it makes.
//! - A function value (DESIGN 27.5): the places of a lambda's body are the places of the function
//!   that wrote it, wherever the lambda is read, and a function written as a value is called by the
//!   lambda that wraps it, so both are in the effect of the function that passes them. A function
//!   that calls a parameter of a function type takes the effect written on the type, and none where
//!   none is written: that parameter's effect goes on to the callers (`through`). Naming a function is
//!   calling it, for the rings.
//! - An effect written on a function is held to the one inferred (E401): `total` to none at all,
//!   `fails <kinds>` to kinds among those written, a ring to `diverges`. A place that waits for an
//!   answer of the checks across the languages is not yet one that cannot be shown, so what is
//!   written is held to the places decided. An effect written on a function type is held to the
//!   effect of each function given to it (E402).

use crate::ast::{Arg, ExprKind, FailKind, FnDecl};
use crate::check::Checked;
use crate::diag::Diag;
use crate::ranges::{Abs, FnFacts, Outcome};
use crate::ty::Ty;
use crate::types::Ref;
use ritsu_base::json::Json;
use ritsu_base::text::{Text, plural};
use std::collections::{BTreeMap, BTreeSet};

/// What a function is inferred to do (DESIGN 4.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    /// The kinds of failure it can end in, waiting places counted.
    pub fails: BTreeSet<FailKind>,
    /// The kinds of the places whose answer has come: what a written effect is held to.
    pub decided: BTreeSet<FailKind>,
    /// Whether it may not stop: it is in a ring of calls, or calls one that may not stop.
    pub diverges: bool,
}

impl Effect {
    pub fn total(&self) -> bool {
        self.fails.is_empty() && !self.diverges
    }

    /// As a function's head would write it: `total`, `fails rule, range`, `diverges`, `fails zero
    /// diverges`.
    pub fn written(&self) -> String {
        written(&self.fails, self.diverges)
    }

    pub fn to_json(&self) -> Json {
        Json::obj([("fails", Json::arr(self.fails.iter().map(|k| Json::str(k.word())))), ("diverges", Json::from(self.diverges))])
    }
}

/// An effect as the head of a function writes it.
pub fn written(fails: &BTreeSet<FailKind>, diverges: bool) -> String {
    let kinds: Vec<&str> = fails.iter().map(|k| k.word()).collect();
    match (kinds.is_empty(), diverges) {
        (true, false) => "total".into(),
        (true, true) => "diverges".into(),
        (false, false) => format!("fails {}", kinds.join(", ")),
        (false, true) => format!("fails {} diverges", kinds.join(", ")),
    }
}

/// The functions of the module a function calls, by their places.
fn callees(c: &Checked, f: &FnDecl) -> BTreeSet<usize> {
    let mut all = Vec::new();
    f.body.walk(&mut all);
    let mut out = BTreeSet::new();
    for e in all {
        if let crate::ast::ExprKind::Call(callee, _) = &e.kind
            && let Some(Ref::Fn(canon, name)) = c.typed.refs.get(callee.id)
            && *canon == c.canon
            && let Some(i) = c.decls.fns.iter().position(|x| x.name.text == *name)
        {
            out.insert(i);
        }
    }
    out
}

/// The rings of calls of a module's functions (its strongly connected components, Tarjan's way),
/// the functions a ring calls before it: the order the analysis goes in. Each ring with whether its
/// calls go round (more than one function, or one that calls itself).
pub fn rings(c: &Checked) -> Vec<(Vec<usize>, bool)> {
    let n = c.module.fns.len();
    let edges: Vec<BTreeSet<usize>> = c.module.fns.iter().map(|f| callees(c, f)).collect();
    struct T<'e> {
        edges: &'e [BTreeSet<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<(Vec<usize>, bool)>,
    }
    impl T<'_> {
        fn visit(&mut self, v: usize) {
            self.index[v] = Some(self.next);
            self.low[v] = self.next;
            self.next += 1;
            self.stack.push(v);
            self.on[v] = true;
            for &w in self.edges[v].iter() {
                match self.index[w] {
                    None => {
                        self.visit(w);
                        self.low[v] = self.low[v].min(self.low[w]);
                    }
                    Some(i) if self.on[w] => self.low[v] = self.low[v].min(i),
                    _ => {}
                }
            }
            if Some(self.low[v]) == self.index[v] {
                let mut ring = Vec::new();
                while let Some(w) = self.stack.pop() {
                    self.on[w] = false;
                    ring.push(w);
                    if w == v {
                        break;
                    }
                }
                ring.sort_unstable();
                let round = ring.len() > 1 || self.edges[v].contains(&v);
                self.out.push((ring, round));
            }
        }
    }
    let mut t = T { edges: &edges, index: vec![None; n], low: vec![0; n], on: vec![false; n], stack: Vec::new(), next: 0, out: Vec::new() };
    for v in 0..n {
        if t.index[v].is_none() {
            t.visit(v);
        }
    }
    t.out
}

/// The effect of every function, ring by ring in the order of [`rings`] (DESIGN 4.3).
pub fn infer(c: &Checked, facts: &mut [FnFacts], rings: &[(Vec<usize>, bool)]) {
    for (ring, round) in rings {
        let mut e = Effect { diverges: *round, ..Effect::default() };
        for &f in ring {
            // its own places, and those its calls' readings again found (DESIGN 26.6)
            for p in facts[f].places.iter().chain(facts[f].rplaces.iter().map(|r| &r.place)) {
                match p.outcome {
                    Outcome::Shown => {}
                    Outcome::Pending => {
                        e.fails.insert(p.kind);
                    }
                    _ => {
                        e.fails.insert(p.kind);
                        e.decided.insert(p.kind);
                    }
                }
            }
            // the calls not read again, and those the readings made
            for call in facts[f].calls.iter().filter(|x| !x.reread).chain(facts[f].rcalls.iter()) {
                let other = callee_effect(c, facts, ring, call);
                if let Some(o) = other {
                    e.fails.extend(o.fails.iter().copied());
                    e.decided.extend(o.decided.iter().copied());
                    e.diverges |= o.diverges;
                }
            }
            // the calls of function values whose type writes an effect (DESIGN 27.5)
            for t in &facts[f].thru {
                e.fails.extend(t.effect.fails.iter().copied());
                e.decided.extend(t.effect.fails.iter().copied());
                e.diverges |= t.effect.diverges;
            }
        }
        for &f in ring {
            facts[f].effect = e.clone();
        }
    }
}

/// The effect of the function a call calls, when it is known by now: one of another module, or one
/// of this module outside the ring (whose effect the ring shares).
fn callee_effect(c: &Checked, facts: &[FnFacts], ring: &[usize], call: &crate::ranges::FnCall) -> Option<Effect> {
    if call.module == c.canon {
        let g = c.decls.fns.iter().position(|x| x.name.text == call.function)?;
        if ring.contains(&g) {
            return None;
        }
        return Some(facts[g].effect.clone());
    }
    let other = c.world.modules.get(&call.module)?;
    let g = other.decls.fns.iter().position(|x| x.name.text == call.function)?;
    other.analysis.get().and_then(|a| a.fns.get(g)).map(|x| x.effect.clone())
}

/// Kinds as a sentence names them: `` `rule` and `range` ``.
fn kinds_text(ks: &BTreeSet<FailKind>) -> Text {
    let parts: Vec<String> = ks.iter().map(|k| format!("`{}`", k.word())).collect();
    match parts.as_slice() {
        [] => Text::default(),
        [one] => Text::same(one.clone()),
        _ => {
            let (last, rest) = parts.split_last().unwrap_or((&parts[0], &[]));
            Text::new(parts.join("、"), format!("{} and {last}", rest.join(", ")))
        }
    }
}

/// E401 for each function whose written effect does not hold what it does: the places and the calls
/// that go past it, as notes.
pub fn written_holds(c: &Checked, facts: &[FnFacts], rings: &[(Vec<usize>, bool)], said: &BTreeMap<(usize, usize), (Text, String)>) -> Vec<Diag> {
    let mut out = Vec::new();
    for (f, decl) in c.module.fns.iter().enumerate() {
        let Some(w) = &decl.effect else { continue };
        let e = &facts[f].effect;
        let allowed: BTreeSet<FailKind> = w.fails.iter().map(|(k, _)| *k).collect();
        let extra: BTreeSet<FailKind> = e.decided.iter().filter(|k| !allowed.contains(k)).copied().collect();
        let loops = e.diverges && !w.diverges;
        if extra.is_empty() && !loops {
            continue;
        }
        let fun = &c.decls.fns[f].name.text;
        let shown = written(&allowed, w.diverges);
        let ks = kinds_text(&extra);
        let msg = match (extra.is_empty(), loops) {
            (false, false) => tr!("関数 `{fun}` には `{shown}` と書いてありますが、{} で失敗することがあります", "`{fun}` is written `{shown}`, but it can fail with {}", ks.ja; ks.en),
            (true, true) => tr!("関数 `{fun}` には `{shown}` と書いてありますが、止まらないことがあります", "`{fun}` is written `{shown}`, but it may not stop"),
            _ => tr!("関数 `{fun}` には `{shown}` と書いてありますが、{} で失敗することがあり、止まらないこともあります", "`{fun}` is written `{shown}`, but it can fail with {} and may not stop", ks.ja; ks.en),
        };
        let mut d = Diag::at("E401", &c.path, decl.line, 1, msg).source(&c.src);
        // the places of its own body that go past it
        for (i, p) in facts[f].places.iter().enumerate() {
            if !extra.contains(&p.kind) || matches!(p.outcome, Outcome::Shown | Outcome::Pending) {
                continue;
            }
            if let Some((what, code)) = said.get(&(f, i)) {
                let line = p.span.line;
                let en = lower_first(&what.en);
                d = d.note(tr!("{line} 行目: {}（{code}）", "Line {line}: {} ({code})", what.ja; en));
            }
        }
        // the places the readings again of its calls found that go past it (DESIGN 26.6)
        let mut read: BTreeSet<(String, usize, &'static str)> = BTreeSet::new();
        for r in &facts[f].rplaces {
            let p = &r.place;
            if !extra.contains(&p.kind) || matches!(p.outcome, Outcome::Shown | Outcome::Pending) {
                continue;
            }
            if read.insert((r.function.clone(), p.span.line, p.kind.word())) {
                let (g, gl, k) = (&r.function, p.span.line, p.kind.word());
                let lines: Vec<usize> = facts[f].calls.iter().filter(|x| x.reread && x.function == *g).map(|x| x.line).collect();
                let at = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ");
                d = d.note(tr!(
                    "{at} 行目: `{g}` を呼んでいます。この呼び出しが渡す値で `{g}` を読み直すと、その {gl} 行目で `{k}` で失敗することがあります",
                    "Line {at}: it calls `{g}`, which, read again with the values the call gives, can fail with `{k}` at its line {gl}"
                ));
            }
        }
        // the calls that bring it
        let ring = rings.iter().find(|(r, _)| r.contains(&f)).map(|(r, _)| r.clone()).unwrap_or_default();
        let mut named = BTreeSet::new();
        for call in facts[f].calls.iter().filter(|x| !x.reread) {
            let g = &call.function;
            let line = call.line;
            let same = call.module == c.canon;
            let in_ring = same && c.decls.fns.iter().position(|x| x.name.text == *g).is_some_and(|i| ring.contains(&i));
            if in_ring {
                if loops && named.insert((line, g.clone())) {
                    d = d.note(tr!("{line} 行目: `{g}` を呼び、呼び出しが輪になっています", "Line {line}: it calls `{g}`, and the calls go round"));
                }
                continue;
            }
            let Some(o) = callee_effect(c, facts, &ring, call) else { continue };
            let theirs: BTreeSet<FailKind> = o.decided.iter().filter(|k| extra.contains(k)).copied().collect();
            if (!theirs.is_empty() || (loops && o.diverges)) && named.insert((line, g.clone())) {
                let eff = written(&o.fails, o.diverges);
                d = d.note(tr!("{line} 行目: `{g}` を呼んでいます。`{g}` のエフェクトは `{eff}` です", "Line {line}: it calls `{g}`, whose effect is `{eff}`"));
            }
        }
        // the calls of function values whose type writes an effect (DESIGN 27.5)
        let mut typed: BTreeSet<(usize, String)> = BTreeSet::new();
        for t in &facts[f].thru {
            let theirs = t.effect.fails.iter().any(|k| extra.contains(k)) || (loops && t.effect.diverges);
            let eff = t.effect.shown();
            if theirs && typed.insert((t.line, eff.clone())) {
                let line = t.line;
                d = d.note(tr!("{line} 行目: 関数の型に `{eff}` と書いた関数の値を呼んでいます", "Line {line}: it calls a function value whose type writes `{eff}`"));
            }
        }
        // the calls the readings again made: their effects come with the call read again
        let mut through: BTreeSet<String> = BTreeSet::new();
        for call in &facts[f].rcalls {
            let Some(o) = callee_effect(c, facts, &ring, call) else { continue };
            let theirs: BTreeSet<FailKind> = o.decided.iter().filter(|k| extra.contains(k)).copied().collect();
            let g = &call.function;
            if (!theirs.is_empty() || (loops && o.diverges)) && through.insert(g.clone()) {
                let eff = written(&o.fails, o.diverges);
                d = d.note(tr!(
                    "読み直した関数の中で `{g}` を呼んでいます。`{g}` のエフェクトは `{eff}` です",
                    "A function read again calls `{g}`, whose effect is `{eff}`"
                ));
            }
        }
        if !extra.is_empty() {
            d = d.note(tr!(
                "書いたエフェクトに収まるよう、失敗しうる場所を示せるようにする（渡す値を `if` で範囲に狭める）か、エフェクトを広げて書いてください。",
                "Make the places shown (narrow the values with an `if` before they go there), or write a wider effect."
            ));
        }
        if loops {
            d = d.note(tr!(
                "呼び出しが輪になる関数には `diverges` と書いてください。kaname は、再帰が止まることを確かめません。",
                "Write `diverges` on a function whose calls go round: kaname does not check that a recursion stops."
            ));
        }
        out.push(d);
    }
    out
}

/// What the line that says a module passes says of its functions (DESIGN 11.1): `1 function,
/// total`, `3 functions: 2 total; 1 fails range`.
pub fn summary(c: &Checked) -> Text {
    let n = c.decls.fns.len();
    let en_n = plural(n, "function", "functions");
    let Some(a) = c.analysis.get() else { return tr!("関数 {n} 個", "{en_n}") };
    if n == 0 {
        return tr!("関数 {n} 個", "{en_n}");
    }
    let mut groups: Vec<(String, usize)> = Vec::new();
    for f in &a.fns {
        let w = f.effect.written();
        match groups.iter_mut().find(|(x, _)| *x == w) {
            Some((_, k)) => *k += 1,
            None => groups.push((w, 1)),
        }
    }
    if let [(w, _)] = groups.as_slice() {
        return tr!("関数 {n} 個、{w}", "{en_n}, {w}");
    }
    let ja = groups.iter().map(|(w, k)| format!("{w} {k} 個")).collect::<Vec<_>>().join("、");
    let en = groups.iter().map(|(w, k)| format!("{k} {w}")).collect::<Vec<_>>().join("; ");
    Text::new(format!("関数 {n} 個: {ja}"), format!("{en_n}: {en}"))
}

/// The functions of a module with their effects, for `--format json` (DESIGN 4.5): each with what is
/// inferred, what is written, and the places that are not shown.
pub fn to_json(c: &Checked) -> Json {
    let Some(a) = c.analysis.get() else { return Json::Null };
    Json::arr(c.decls.fns.iter().enumerate().map(|(f, info)| {
        let facts = &a.fns[f];
        let written = c.module.fns[f].effect.as_ref().map(|w| {
            let ks: BTreeSet<FailKind> = w.fails.iter().map(|(k, _)| *k).collect();
            Json::obj([("total", Json::from(w.total)), ("fails", Json::arr(ks.iter().map(|k| Json::str(k.word())))), ("diverges", Json::from(w.diverges))])
        });
        let places = facts.places.iter().filter(|p| p.failing()).map(|p| {
            let outcome = match p.outcome {
                Outcome::Example(_) => "example",
                Outcome::Undecided => "undecided",
                Outcome::Unbounded => "unbounded",
                Outcome::Pending => "waiting",
                Outcome::Shown => "shown",
            };
            Json::obj([
                ("line", Json::int(p.span.line as i128)),
                ("kind", Json::str(p.kind.word())),
                ("check", Json::str(p.what.across().unwrap_or("kaname"))),
                ("code", Json::opt_str(p.across.as_ref().map(|(code, _)| code.clone()).or_else(|| crate::ranges::code_of(p).map(str::to_string)))),
                ("outcome", Json::str(outcome)),
            ])
        });
        let calls = facts.calls.iter().map(|x| Json::obj([("line", Json::int(x.line as i128)), ("function", Json::str(x.function.clone()))]));
        let mut o: Vec<(String, Json)> = vec![("function".into(), Json::str(info.name.text.clone()))];
        // an inner function, which no host calls (DESIGN 21.3)
        if info.inner() {
            o.push(("inner".into(), Json::from(true)));
        }
        o.extend([("effect".into(), effect_json(info, &facts.effect)), ("written".into(), written.unwrap_or(Json::Null)), ("places".into(), Json::arr(places)), ("calls".into(), Json::arr(calls))]);
        Json::Obj(o)
    }))
}

/// The parameters of a function type with no effect written on it (DESIGN 27.5): the effect of what is
/// given to them goes on to the function's callers.
pub fn through(info: &crate::names::FnInfo) -> Vec<String> {
    info.params.iter().filter(|p| matches!(p.slot.ty, Ty::Fn(_, _, None))).map(|p| p.name.text.clone()).collect()
}

/// A function's effect for `--format json` and `api`: with `through`, the parameters whose functions'
/// effects go on to the callers, for a function that takes functions (DESIGN 27.5).
pub fn effect_json(info: &crate::names::FnInfo, e: &Effect) -> Json {
    let Json::Obj(mut o) = e.to_json() else { return e.to_json() };
    if info.takes_fn() {
        o.push(("through".into(), Json::arr(through(info).into_iter().map(Json::str))));
    }
    Json::Obj(o)
}

/// E402 for each function given to a parameter whose function type writes an effect, whose own effect
/// goes past it (DESIGN 27.2, 27.5): a lambda's is the kinds of its body's places that are not shown and
/// the effects of the functions it calls; a function value the analysis does not see (a parameter) has
/// the effect its type writes, and any where it writes none.
pub fn passed_hold(c: &Checked, facts: &[FnFacts], at: &[Option<Abs>], rings: &[(Vec<usize>, bool)]) -> Vec<Diag> {
    let mut out = Vec::new();
    for (f, decl) in c.module.fns.iter().enumerate() {
        let ring = rings.iter().find(|(r, _)| r.contains(&f)).map(|(r, _)| r.clone()).unwrap_or_default();
        let mut all = Vec::new();
        decl.body.walk(&mut all);
        for e in all {
            let ExprKind::Call(callee, args) = &e.kind else { continue };
            let Some(Ref::Fn(canon, name)) = c.typed.refs.get(callee.id) else { continue };
            let owner = if *canon == c.canon { Some(&c.decls) } else { c.world.modules.get(canon).map(|m| &m.decls) };
            let Some(info) = owner.and_then(|d| d.fns.iter().find(|x| x.name.text == *name)) else { continue };
            for a in args {
                let Arg::Named(n, _, x) = a else { continue };
                let Some(p) = info.params.iter().find(|p| p.name.is(n)) else { continue };
                let Ty::Fn(_, _, Some(w)) = &p.slot.ty else { continue };
                let Some(Some(Abs::Fns(lams, unseen))) = at.get(x.id) else { continue };
                let mut fails: BTreeSet<FailKind> = BTreeSet::new();
                let mut diverges = false;
                let mut any = false;
                match unseen {
                    Some(Some(u)) => {
                        fails.extend(u.fails.iter().copied());
                        diverges |= u.diverges;
                    }
                    Some(None) => any = true,
                    None => {}
                }
                for l in lams.iter().filter(|l| l.module == c.canon) {
                    let (k, d) = lambda_effect(c, facts, &ring, f, l.expr);
                    fails.extend(k);
                    diverges |= d;
                }
                let allowed: BTreeSet<FailKind> = w.fails.iter().copied().collect();
                let extra: BTreeSet<FailKind> = fails.iter().filter(|k| !allowed.contains(k)).copied().collect();
                let loops = diverges && !w.diverges;
                if extra.is_empty() && !loops && !any {
                    continue;
                }
                let (pn, fun, ws) = (&p.name.text, &info.name.text, w.shown());
                let mut d = Diag::at("E402", &c.path, x.span.line, x.span.col, tr!(
                    "`{fun}` の引数 `{pn}` の型には `{ws}` と書いてありますが、渡した関数はそれに収まりません",
                    "The parameter `{pn}` of `{fun}` writes `{ws}` on its type, and the function given does more"
                ))
                .source(&c.src);
                if any {
                    d = d.note(tr!(
                        "渡しているのは、エフェクトを型に書いていない関数の型の引数です。その関数のエフェクトは、呼ぶ側が渡すものによります。引数の型にも `{ws}` のようにエフェクトを書いてください。",
                        "What is given is a parameter of a function type with no effect written on it, whose effect is what its callers give: write the effect on its type too, as `{ws}`."
                    ));
                } else {
                    let eff = written(&fails, diverges);
                    d = d.note(tr!("渡した関数のエフェクトは `{eff}` です。", "The function given is `{eff}`."));
                }
                d = d.note(tr!(
                    "失敗しうる場所を示せるようにする（ラムダの中で値を `if` で範囲に狭める）か、引数の型に書いたエフェクトを広げてください。",
                    "Make the places of the function given shown (narrow the values with an `if` in the lambda), or widen the effect written on the parameter's type."
                ));
                out.push(d);
            }
        }
    }
    out
}

/// The effect of the lambda `id` written in the function `f` (DESIGN 27.5): the kinds of the places of
/// its body that are not shown (decided or waiting), those of the functions its body calls and of the
/// function values it calls whose type writes one, and whether it may not stop (a call in the ring of
/// `f`, or of a function that may not stop).
fn lambda_effect(c: &Checked, facts: &[FnFacts], ring: &[usize], f: usize, id: usize) -> (BTreeSet<FailKind>, bool) {
    let mut fails = BTreeSet::new();
    let mut diverges = false;
    let Some((_, lam)) = c.lambda(id) else { return (fails, diverges) };
    let mut es = Vec::new();
    lam.walk(&mut es);
    let ids: BTreeSet<usize> = es.iter().map(|e| e.id).collect();
    for p in facts[f].places.iter() {
        if ids.contains(&p.expr) && !matches!(p.outcome, Outcome::Shown) {
            fails.insert(p.kind);
        }
    }
    for call in facts[f].calls.iter().filter(|x| ids.contains(&x.expr)) {
        let in_ring = call.module == c.canon && c.decls.fns.iter().position(|x| x.name.text == call.function).is_some_and(|g| ring.contains(&g));
        if in_ring {
            diverges = true;
            continue;
        }
        if let Some(o) = callee_effect(c, facts, ring, call) {
            fails.extend(o.fails.iter().copied());
            diverges |= o.diverges;
        }
    }
    for t in facts[f].thru.iter().filter(|t| ids.contains(&t.expr)) {
        fails.extend(t.effect.fails.iter().copied());
        diverges |= t.effect.diverges;
    }
    (fails, diverges)
}

/// An English sentence put after a colon: its first word lowercased, unless it is a name (`isqrt`)
/// or an acronym.
fn lower_first(s: &str) -> String {
    let mut cs = s.chars();
    match (cs.next(), s.chars().nth(1)) {
        (Some(c), Some(d)) if c.is_ascii_uppercase() && d.is_ascii_lowercase() => c.to_ascii_lowercase().to_string() + cs.as_str(),
        _ => s.to_string(),
    }
}
