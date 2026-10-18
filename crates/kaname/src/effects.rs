//! The effects of a module's functions (DESIGN 4): which kinds of failure each can end in, and
//! whether it may not stop.
//!
//! - A function's own kinds are those of the places in its body that are not shown (`ranges.rs`):
//!   an example, an undecided place, and a place that waits for the checks across the languages
//!   (ritsu-cross's X2, X3, X4, X6), which is counted as undecided until its answer comes.
//! - A function takes the effect of each function it calls, and the functions of a ring of calls
//!   (a strongly connected component of the calls, or a function that calls itself) share the kinds
//!   of all of them and may not stop (DESIGN 4.3).
//! - An effect written on a function is held to the one inferred (E401): `total` to none at all,
//!   `fails <kinds>` to kinds among those written, a ring to `diverges`. A place that waits for an
//!   answer of the checks across the languages is not yet one that cannot be shown, so what is
//!   written is held to the places decided.

use crate::ast::{FailKind, FnDecl};
use crate::check::Checked;
use crate::diag::Diag;
use crate::ranges::{FnFacts, Outcome};
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
            for p in &facts[f].places {
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
            for call in &facts[f].calls {
                let other = callee_effect(c, facts, ring, call);
                if let Some(o) = other {
                    e.fails.extend(o.fails.iter().copied());
                    e.decided.extend(o.decided.iter().copied());
                    e.diverges |= o.diverges;
                }
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
        // the calls that bring it
        let ring = rings.iter().find(|(r, _)| r.contains(&f)).map(|(r, _)| r.clone()).unwrap_or_default();
        let mut named = BTreeSet::new();
        for call in &facts[f].calls {
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
        Json::obj([
            ("function", Json::str(info.name.text.clone())),
            ("effect", facts.effect.to_json()),
            ("written", written.unwrap_or(Json::Null)),
            ("places", Json::arr(places)),
            ("calls", Json::arr(calls)),
        ])
    }))
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
