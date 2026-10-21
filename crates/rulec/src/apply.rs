//! `apply` (§15.69): a rule applied with its inputs bound — a provision
//! applied mutatis mutandis. The callee is read at the same revision, held to its pinned
//! digest, checked on its own, and then expanded into the applying rule: its definitions are
//! renamed `<apply>:<name>`, its inputs are replaced by what they are bound to, and its
//! outputs become definitions of this rule. Everything downstream — the type check, the
//! definition sets, the evaluator, the generators, the page — sees one rule.
//!
//! What the expansion cannot prove syntactically, `check` proves once this rule is typed: that
//! every bound value stays inside the callee's declared range and that the callee's
//! constraints follow from this rule's declarations (E043), and that the types agree (E042).

use crate::ast::*;
use crate::diag::{Diag, Severity, Span};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::HashMap;
use std::path::Path;

/// The heading as `rulec source pin` writes it and as the fix offers it: pinned at the callee's
/// meaning (§15.230), which a comment does not move.
pub fn header_line(a: &ApplyDecl, meaning: &str) -> String {
    let name = match &a.name.ascii {
        Some(al) => format!("{}({al})", a.name.text),
        None => a.name.text.clone(),
    };
    format!("{} {name} = \"{}\" meaning:{meaning}", crate::kw::APPLY, a.path)
}

/// What a callee input is replaced by.
#[derive(Clone)]
enum Sub {
    Name(String),
    Lit(Lit),
}

struct Renamer<'a> {
    apply: &'a str,
    alias: String,
    /// The `apply` heading: where every diagnostic about the inlined items points, since the
    /// callee's own positions mean nothing in this file.
    at: Span,
    /// Callee input → what it is bound to.
    subs: HashMap<String, Sub>,
    /// Callee enum input → (callee value → this rule's values that stand for it).
    maps: HashMap<String, HashMap<String, Vec<String>>>,
    /// Callee groups, expanded to their members.
    groups: HashMap<String, Vec<String>>,
    /// Callee enum input bound to a name of this rule → the values of that name's enum, in their
    /// order: what a cell that takes the absent value besides some of them is written against
    /// (§15.255).
    own: HashMap<String, Vec<String>>,
    /// Callee input → the enum it has, when it is an enum.
    input_enum: HashMap<String, String>,
    /// The callee's enums and what they are called here (§15.230).
    enum_names: Vec<(String, String)>,
}

impl Renamer<'_> {
    fn text(&self, n: &str) -> String {
        format!("{}:{n}", self.apply)
    }
    /// A word the callee writes for a column: a field or an input bound to a name of this rule,
    /// `held` as it is, any other name of the callee's under this apply.
    fn word(&self, w: &str) -> String {
        match self.subs.get(w) {
            Some(Sub::Name(n)) => n.clone(),
            _ if w == crate::kw::HELD => w.to_string(),
            _ => self.text(w),
        }
    }
    fn sp(&self) -> Span {
        self.at.clone()
    }
    fn name(&self, n: &Name) -> Name {
        Name {
            text: self.text(&n.text),
            ascii: Some(format!("{}_{}", self.alias, n.ascii.clone().unwrap_or_else(|| n.text.clone()))),
            span: self.sp(),
        }
    }
    /// A word in an expression: a bound input, a callee name, or a literal word.
    fn expr_word(&self, w: &str) -> Expr {
        match self.subs.get(w) {
            Some(Sub::Name(n)) => Expr::Name(n.clone(), self.sp()),
            Some(Sub::Lit(l)) => Expr::Lit(l.clone(), self.sp()),
            None if w == crate::kw::TRUE || w == crate::kw::FALSE || w == crate::kw::HELD => Expr::Name(w.to_string(), self.sp()),
            None => Expr::Name(self.text(w), self.sp()),
        }
    }
    fn expr(&self, e: &Expr) -> Expr {
        match e {
            Expr::Name(w, _) => self.expr_word(w),
            Expr::Lit(l, _) => Expr::Lit(l.clone(), self.sp()),
            Expr::Bin(a, op, b, _) => Expr::Bin(Box::new(self.expr(a)), *op, Box::new(self.expr(b)), self.sp()),
            Expr::Call(f, args, _) => Expr::Call(f.clone(), args.iter().map(|x| self.expr(x)).collect(), self.sp()),
        }
    }
    /// The words a cell of a bound enum column matches, in this rule's vocabulary. `None`
    /// when a word maps to nothing, which makes the row dead.
    fn map_words(&self, input: &str, ls: &[Lit]) -> Vec<String> {
        let map = self.maps.get(input);
        let mut out: Vec<String> = Vec::new();
        for l in ls {
            let Lit::Word(w) = l else { continue };
            let members: Vec<String> = match self.groups.get(w) {
                Some(ms) => ms.clone(),
                None => vec![w.clone()],
            };
            for m in members {
                let targets: Vec<String> = match map {
                    Some(mp) => mp.get(&m).cloned().unwrap_or_default(),
                    None => vec![m.clone()],
                };
                for t in targets {
                    if !out.contains(&t) {
                        out.push(t);
                    }
                }
            }
        }
        out
    }
    /// The values of this rule's enum a callee's enum input is bound to, in their order.
    fn own_values(&self, input: &str) -> Vec<String> {
        self.own.get(input).cloned().unwrap_or_default()
    }
    /// The cell of this rule that takes `words` — this rule's values for a callee's cell, `none`
    /// among them where `with none -> …` reads the absent value as one of the callee's (§15.255).
    /// No cell names `none` beside values, and `not:` takes the absent value too, so a cell that
    /// takes it and some values is `not:` the others. `None` where it takes nothing.
    fn cell_of_words(&self, input: &str, words: Vec<String>) -> Option<Cell> {
        if !words.iter().any(|w| w == crate::kw::NONE) {
            return match words.len() {
                0 => None,
                1 => Some(Cell::Lit(Lit::Word(words[0].clone()))),
                _ => Some(Cell::Set(words.into_iter().map(Lit::Word).collect())),
            };
        }
        let values: Vec<&String> = words.iter().filter(|w| *w != crate::kw::NONE).collect();
        if values.is_empty() {
            return Some(Cell::Nothing);
        }
        let rest: Vec<Lit> = self.own_values(input).into_iter().filter(|v| !values.contains(&v)).map(Lit::Word).collect();
        Some(if rest.is_empty() { Cell::DontCare } else { Cell::Not(rest) })
    }
    /// The cell of this rule for the callee's `not:` cell whose words stand for `words` here: every
    /// value but those, and the absent value only where it is not among them (§15.255).
    fn not_cell_of_words(&self, input: &str, words: Vec<String>) -> Option<Cell> {
        if words.is_empty() {
            return Some(Cell::DontCare);
        }
        if !words.iter().any(|w| w == crate::kw::NONE) {
            return Some(Cell::Not(words.into_iter().map(Lit::Word).collect()));
        }
        let rest: Vec<String> = self.own_values(input).into_iter().filter(|v| !words.contains(v)).collect();
        match rest.len() {
            0 => None,
            1 => Some(Cell::Lit(Lit::Word(rest[0].clone()))),
            _ => Some(Cell::Set(rest.into_iter().map(Lit::Word).collect())),
        }
    }
    /// A type the callee writes, its enum named as it is here.
    fn ty(&self, t: &TypeRef) -> TypeRef {
        match self.enum_names.iter().find(|(c, _)| *c == t.base) {
            Some((_, mine)) => TypeRef { base: mine.clone(), ..t.clone() },
            None => t.clone(),
        }
    }
    fn cite(&self, c: &Option<Cite>) -> Option<Cite> {
        c.as_ref().map(|c| Cite { source: self.text(&c.source), fragments: c.fragments.clone(), span: self.sp() })
    }
}

/// Expand every `apply` of `f` in place. Errors are the callee not being usable (E044), its
/// pin (E040) and the shape of the bindings (E041, E035); what needs this rule's types is
/// left to [`check`].
/// Does the applying rule declare this name? `expand` runs before the rule is typed, so the
/// question is asked of the source. It decides whether a bare word in a binding is a name of
/// this rule or a literal of the callee's (DESIGN §15.90); a name always wins, which is the
/// order `check` uses too.
fn declares(f: &RuleFile, n: &str) -> bool {
    f.inputs.iter().any(|i| i.name.text == n)
        || f.outputs.iter().any(|o| o.name.text == n)
        || f.element_fields().any(|i| i.name.text == n)
        || f.items.iter().any(|it| match it {
            Item::Derived(d) => d.name.text == n,
            Item::Define(d) => d.name.text == n,
            Item::Agg(d) => d.name.text == n,
            Item::Fold(d) => d.answer.as_ref().is_some_and(|a| a.name.text == n),
            Item::Table(t) => t.outputs.iter().any(|o| o.name.text == n),
        })
}

thread_local! {
    /// The rules being expanded on this thread, outermost first: a callee already among them is a
    /// ring of applies (§15.230).
    static EXPANDING: std::cell::RefCell<Vec<(std::path::PathBuf, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn canon(p: &Path) -> std::path::PathBuf {
    ritsu_base::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// A ring of applies below `p`, by its files' `apply` lines alone: `chain` is the way from the
/// rule entered to `p`, `seen` the rules on it. The ring is the files from where it starts.
fn ring_below(p: &Path, seen: &mut Vec<std::path::PathBuf>, chain: &mut Vec<String>) -> Option<Vec<String>> {
    if chain.len() > 32 {
        return None;
    }
    let me = canon(p);
    if seen.contains(&me) {
        return Some(chain.clone());
    }
    seen.push(me);
    let src = crate::vfs::read_to_string(p).ok()?;
    let f = crate::parse::parse(&src, &p.to_string_lossy()).file?;
    let dir = p.parent().unwrap_or(Path::new(".")).to_path_buf();
    for a in &f.applies {
        let q = dir.join(&a.path);
        chain.push(q.to_string_lossy().into_owned());
        if let Some(r) = ring_below(&q, seen, chain) {
            return Some(r);
        }
        chain.pop();
    }
    seen.pop();
    None
}

pub fn expand(f: &mut RuleFile, path: &str) -> Vec<Diag> {
    if f.applies.is_empty() {
        return Vec::new();
    }
    EXPANDING.with(|e| e.borrow_mut().push((canon(Path::new(path)), path.to_string())));
    let out = expand_inner(f, path);
    EXPANDING.with(|e| {
        e.borrow_mut().pop();
    });
    out
}

fn expand_inner(f: &mut RuleFile, path: &str) -> Vec<Diag> {
    let mut out: Vec<Diag> = Vec::new();
    let dir = Path::new(path).parent().unwrap_or(Path::new(".")).to_path_buf();
    let applies = std::mem::take(&mut f.applies);
    let mut done: Vec<ApplyDecl> = Vec::new();
    let mut shift = 0usize;
    for mut a in applies {
        a.at += shift;
        let an = a.name.text.clone();
        let at = |line: usize| tr!("{path}:{line} 準用 {an}", "{path}:{line} apply {an}");
        let e044 = |what: String, note: String| {
            Diag::error("E044", tr!("`{an}` を準用できません: {what}", "`{an}` cannot be applied: {what}"))
                .at(at(a.span.line))
                .mark(a.span.clone(), "")
                .note(note)
        };

        // The callee, at the same revision as this rule.
        let cpath = dir.join(&a.path);
        let bytes = match crate::vfs::read(&cpath) {
            Ok(b) => b,
            Err(_) => {
                out.push(e044(
                    tr!("`{}` を読めません", "`{}` cannot be read", a.path),
                    tr!("パスは規則ファイルのある場所からたどります（探した先: {}）。", "The path is followed from the directory of the rule file (looked for: {}).", cpath.display()),
                ));
                done.push(a);
                continue;
            }
        };
        let h = crate::sha256::short(&bytes);
        let src = String::from_utf8_lossy(&bytes).into_owned();
        // A callee that applies another is expanded as its own check expands it (§15.230). A
        // callee already being expanded on the way here is a ring, said whole, before anything
        // reads through it — and before its pin, which no file of a ring can hold, since each
        // one's meaning holds the pins of the next.
        let ring: Option<Vec<String>> = EXPANDING.with(|e| {
            let st = e.borrow();
            let me = canon(&cpath);
            st.iter().position(|(k, _)| *k == me).map(|i| st[i..].iter().map(|(_, p)| p.clone()).chain(std::iter::once(cpath.to_string_lossy().into_owned())).collect())
        });
        // A ring further down, through the callee's own applies, is said here too, where it is
        // entered: read by the paths alone, before anything is checked.
        let ring = ring.or_else(|| {
            let mut seen: Vec<std::path::PathBuf> = EXPANDING.with(|e| e.borrow().iter().map(|(k, _)| k.clone()).collect());
            let mut chain = vec![path.to_string(), cpath.to_string_lossy().into_owned()];
            ring_below(&cpath, &mut seen, &mut chain)
        });
        if let Some(ring) = ring {
            out.push(e044(
                tr!("準用が輪になっています（{}）", "the applies go round ({})", ring.join(" → ")),
                tr!(
                    "ある規則を準用する規則を、その規則が（ほかの規則を通して）準用しています。どの規則も、自分を通して自分を読めません。どこか一つの `apply` を外してください。",
                    "A rule applies a rule that applies it back, directly or through others; no rule can read itself through itself. Take one of the `apply` lines out."
                ),
            ));
            done.push(a);
            continue;
        }
        // The pin (§15.230): `meaning:` holds the callee's lines with its comments, description and
        // citations left out, so a comment does not move it; a `sha256:` written before is read as
        // it was, the file's bytes.
        let m = crate::link::pin::meaning(&src);
        let (pinned, now, word) = match (&a.meaning, &a.hash) {
            (Some(p), _) => (Some(p.clone()), m.clone(), "meaning"),
            (None, Some(p)) => (Some(p.clone()), h.clone(), "sha256"),
            (None, None) => (None, m.clone(), "meaning"),
        };
        match &pinned {
            None => out.push(
                Diag::error("E040", tr!("元の規則 `{}` の意味が固定されていません", "The meaning of the callee `{}` is not pinned", a.path))
                    .at(at(a.span.line))
                    .mark(a.span.clone(), "")
                    .note(tr!("いまの元の規則は meaning:{m} です。この内容で承認するなら、見出しを次のとおりにしてください（`rulec source pin` でも書けます）。", "The callee is meaning:{m} now. To pin it as the one approved, make the heading the following (`rulec source pin` writes it too)."))
                    .fix(crate::diag::FixKind::PinSource, header_line(&a, &m)),
            ),
            Some(p) if *p != now => {
                let mut d = Diag::error("E040", tr!("元の規則 `{}` が変わっています", "The callee `{}` has changed", a.path))
                    .at(at(a.span.line))
                    .mark(a.span.clone(), tr!("固定: {word}:{p}", "pinned: {word}:{p}"))
                    .note(tr!("いまの元の規則: {word}:{now}", "The callee now: {word}:{now}"));
                // What moved since the pin, where the repository has the version it was taken at:
                // the lines that differ, comments and citations left out.
                if word == "meaning" {
                    if let Some(old) = crate::link::pin::meaning_at(&cpath.to_string_lossy(), p) {
                        let new = crate::link::pin::meaning_pairs(&src);
                        for n in crate::link::pin::moved_pairs(&old, &new).into_iter().take(12) {
                            d = d.note(n);
                        }
                    }
                }
                out.push(
                    d.note(tr!("`rulec diff` で、この規則の答えが何件、どれだけ動くかを見てから、見出しを次のとおり書き換えて固定し直してください。", "See with `rulec diff` how many answers of this rule move and by how much, then rewrite the heading as follows to pin the new callee."))
                        .fix(crate::diag::FixKind::PinSource, header_line(&a, &m)),
                );
            }
            _ => {}
        }
        let cp = cpath.to_string_lossy().into_owned();
        a.callee_src = src.clone();
        a.callee_path = cp.clone();
        // The callee has to pass the whole check — completeness and the rest, not the type
        // check alone — before its definitions are trusted here. It is checked as it is: the
        // ranges `outputs_over` holds this rule's inputs to are not the callee's.
        let (verdict, prepared) = crate::over::without(|| (crate::report(&src, &cp).diags, crate::prepare(&src, &cp)));
        let (cf, cc) = match prepared {
            Ok(v) if !verdict.iter().any(|d| d.severity == Severity::Error) => v,
            _ => {
                let mut codes: Vec<&str> = verdict.iter().filter(|d| d.severity == Severity::Error).map(|d| d.code).collect();
                codes.sort();
                codes.dedup();
                out.push(e044(
                    tr!("`{}` が check を通りません（{}）", "`{}` does not pass check ({})", a.path, codes.join(", ")),
                    tr!("先に元の規則を直してください。通らない規則を準用しても、通らない規則になるだけです。", "Fix the callee first. A broken rule expanded is a broken rule."),
                ));
                done.push(a);
                continue;
            }
        };
        let cal_alias = cf.name.ascii.clone().unwrap_or_else(|| an.clone());

        // The bindings: every callee input exactly once, nothing the callee lacks.
        let mut bad = false;
        // A callee that walks a sequence (§15.230): each of its sequences is read as one of this
        // rule's, `<callee sequence> = <this rule's sequence>`, its fields by name, or as `with
        // <this rule's field> -> <callee's field>` says. Its summaries and folds then walk this
        // rule's sequence.
        let mut seq_subs: Vec<(String, String)> = Vec::new();
        let mut field_subs: Vec<(String, String)> = Vec::new();
        let mut field_pairs: Vec<(String, CalleeInput)> = Vec::new();
        for el in &cf.elements {
            let n = el.name.text.as_str();
            let bs: Vec<&Binding> = a.bindings.iter().filter(|b| b.input == n).collect();
            let Some(b) = bs.first().filter(|_| bs.len() == 1) else {
                out.push(
                    Diag::error("E041", if bs.is_empty() {
                        tr!("元の規則の並び `{n}` の読み替えがありません", "The callee's sequence `{n}` is not bound")
                    } else {
                        tr!("元の規則の並び `{n}` の読み替えが二度あります", "The callee's sequence `{n}` is bound twice")
                    })
                    .at(at(a.span.line))
                    .mark(a.span.clone(), "")
                    .note(tr!(
                        "並びをたどる規則を準用するときは、元の規則の並びを、この規則の並びで `{n} = <この規則の並び>` と読み替えてください。要素のフィールドは名前で対応し、違う名前は `with <この規則のフィールド> -> <元の規則のフィールド>` で書きます。",
                        "To apply a rule that walks a sequence, bind its sequence to one of this rule's: `{n} = <sequence>`. The fields of an element map by name; a field named otherwise is mapped with `with <this rule's field> -> <callee's field>`."
                    )),
                );
                bad = true;
                continue;
            };
            let BindValue::Name(mine) = &b.value else {
                out.push(
                    Diag::error("E041", tr!("並び `{n}` は、この規則の並びで読み替えます", "The sequence `{n}` is bound to a sequence of this rule"))
                        .at(at(b.span.line))
                        .mark(b.span.clone(), "")
                        .note(tr!("この規則の並び: {}", "This rule's sequences: {}", f.elements.iter().map(|e| e.name.text.clone()).collect::<Vec<_>>().join(", "))),
                );
                bad = true;
                continue;
            };
            let Some(mine_el) = f.elements.iter().find(|e| e.name.text == *mine) else {
                out.push(
                    Diag::error("E041", tr!("`{mine}` はこの規則の並びではありません", "`{mine}` is no sequence of this rule"))
                        .at(at(b.span.line))
                        .mark(b.span.clone(), "")
                        .note(tr!("この規則の並び: {}", "This rule's sequences: {}", f.elements.iter().map(|e| e.name.text.clone()).collect::<Vec<_>>().join(", "))),
                );
                bad = true;
                continue;
            };
            seq_subs.push((n.to_string(), mine.clone()));
            for cfd in &el.fields {
                let ours = b.map.iter().find(|(_, to)| *to == cfd.name.text).map(|(from, _)| from.clone()).unwrap_or_else(|| cfd.name.text.clone());
                match mine_el.fields.iter().find(|x| x.name.text == ours) {
                    Some(_) => {
                        field_subs.push((cfd.name.text.clone(), ours.clone()));
                        field_pairs.push((ours, CalleeInput { name: cfd.name.text.clone(), ty: cc.ty_of(&cfd.name.text).unwrap_or(Ty::Unknown), range: cfd.range.clone(), enum_range: None }));
                    }
                    None => {
                        out.push(
                            Diag::error("E041", tr!("元の規則の要素のフィールド `{}` にあたるものが、並び `{mine}` にありません", "The callee's field `{}` has nothing in the sequence `{mine}`", cfd.name.text))
                                .at(at(b.span.line))
                                .mark(b.span.clone(), "")
                                .note(tr!("`{mine}` のフィールド: {}。違う名前なら `with <この規則のフィールド> -> {}` で対応づけてください。", "The fields of `{mine}`: {}. A field named otherwise is mapped with `with <this rule's field> -> {}`.", mine_el.fields.iter().map(|x| x.name.text.clone()).collect::<Vec<_>>().join(", "), cfd.name.text)),
                        );
                        bad = true;
                    }
                }
            }
        }
        for cin in &cf.inputs {
            let n = cin.name.text.as_str();
            let k = a.bindings.iter().filter(|b| b.input == n).count();
            if k != 1 {
                out.push(
                    Diag::error("E041", if k == 0 {
                        tr!("元の規則の入力 `{n}` の読み替えがありません", "The callee input `{n}` is not bound")
                    } else {
                        tr!("元の規則の入力 `{n}` の読み替えが二度あります", "The callee input `{n}` is bound twice")
                    })
                    .at(at(a.span.line))
                    .mark(a.span.clone(), "")
                    .note(tr!("元の規則の入力は全部、`<元の入力> = <この規則の値>` の行で読み替えを書いてください。", "Every callee input is bound explicitly by a `<input> = <value>` line; that is the substitution.")),
                );
                bad = true;
            }
        }
        for b in &a.bindings {
            if !cf.inputs.iter().any(|i| i.name.text == b.input) && !cf.elements.iter().any(|e| e.name.text == b.input) {
                out.push(
                    Diag::error("E041", tr!("`{}` に `{}` という入力はありません", "`{}` has no input called `{}`", a.path, b.input))
                        .at(at(b.span.line))
                        .mark(b.span.clone(), "")
                        .note(tr!("元の規則の入力: {}", "The callee's inputs: {}", cf.inputs.iter().map(|i| i.name.text.clone()).collect::<Vec<_>>().join(", "))),
                );
                bad = true;
            }
        }
        for ob in &a.outputs {
            if !cf.outputs.iter().any(|o| o.name.text == ob.output) {
                out.push(
                    Diag::error("E041", tr!("`{}` に `{}` という出力はありません", "`{}` has no output called `{}`", a.path, ob.output))
                        .at(at(ob.span.line))
                        .mark(ob.span.clone(), "")
                        .note(tr!("元の規則の出力: {}", "The callee's outputs: {}", cf.outputs.iter().map(|o| o.name.text.clone()).collect::<Vec<_>>().join(", "))),
                );
                bad = true;
            }
        }
        // The callee's definitions an `except` names.
        let mut dropped_tables: Vec<String> = Vec::new();
        let mut dropped_rows: Vec<(String, String)> = Vec::new();
        for (target, sp) in &a.excepts {
            let is_table = cf.items.iter().any(|it| matches!(it, Item::Table(t) if t.name.as_ref().is_some_and(|n| n.text == *target)));
            if is_table {
                dropped_tables.push(target.clone());
                continue;
            }
            let row = target.rsplit_once(':').and_then(|(tn, lbl)| {
                cf.items.iter().find_map(|it| match it {
                    Item::Table(t) if t.name.as_ref().is_some_and(|n| n.text == tn) && t.rows.iter().any(|r| r.label.as_ref().is_some_and(|l| l.text == lbl)) => Some((tn.to_string(), lbl.to_string())),
                    _ => None,
                })
            });
            match row {
                Some(r) => dropped_rows.push(r),
                None => {
                    out.push(
                        Diag::error("E035", tr!("`{}` の指す先 `{target}` が `{}` にありません", "The target `{target}` of `{}` is not in `{}`", crate::kw::EXCEPT, a.path))
                            .at(at(sp.line))
                            .mark(sp.clone(), "")
                            .note(tr!("書けるのは元の規則の表か節の名前、または `表:行ラベル` です。行を指すにはラベルが要ります。", "A target is a table or clause of the callee, or `table:label`. A row needs a label.")),
                    );
                    bad = true;
                }
            }
        }
        // The enums the callee declares (§15.230) come in as `<apply>:<enum>`, values spelled as the
        // callee spells them; one the callee reads from an `enums` file this rule reads too is that
        // one, with no copy.
        let mut enum_names: Vec<(String, String)> = Vec::new();
        for e in &cf.enums {
            let shared = e.from.as_ref().and_then(|o| {
                f.enums.iter().find(|m| m.from.as_ref().is_some_and(|mo| mo.path == o.path && mo.name.text == o.name.text)).map(|m| m.name.text.clone())
            });
            match shared {
                Some(mine) => enum_names.push((e.name.text.clone(), mine)),
                None => {
                    let copy = format!("{an}:{}", e.name.text);
                    enum_names.push((e.name.text.clone(), copy));
                }
            }
        }
        if bad {
            done.push(a);
            continue;
        }

        // The divisions of a country (§15.182). A binding may name one in any of its
        // spellings, and both sides of a `with` too; read as the values they stand for, they
        // are what the rest of this function and `check` compare. The same country imported
        // under its other name here (`std/都道府県` against `std/jp/prefectures`) maps division
        // by division, as a value spelled the same on both sides does.
        for b in &mut a.bindings {
            let Some(cin) = cf.inputs.iter().find(|i| i.name.text == b.input) else { continue };
            let Some(theirs) = crate::prelude::import_of(&cf, &cin.ty.base) else { continue };
            match &mut b.value {
                BindValue::Name(w) | BindValue::Lit(Lit::Word(w)) if !declares(f, w) => {
                    if let Some(v) = theirs.resolve(w) {
                        *w = v.to_string();
                    }
                }
                _ => {}
            }
            let ours = match &b.value {
                BindValue::Name(n) if declares(f, n) => caller_import(f, n),
                _ => None,
            };
            for (from, to) in &mut b.map {
                if let Some(v) = theirs.resolve(to) {
                    *to = v.to_string();
                }
                if let Some(v) = ours.and_then(|im| im.resolve(from)) {
                    *from = v.to_string();
                }
            }
            if let Some(ours) = ours.filter(|o| o.ns.country == theirs.ns.country && o.japanese != theirs.japanese) {
                for d in ours.ns.divisions {
                    let from = ours.value(d).to_string();
                    if !b.map.iter().any(|(x, _)| *x == from) {
                        b.map.push((from, theirs.value(d).to_string()));
                    }
                }
            }
        }

        // The renamer: subs, maps, groups.
        let mut subs: HashMap<String, Sub> = HashMap::new();
        let mut maps: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
        let mut own: HashMap<String, Vec<String>> = HashMap::new();
        let mut input_enum: HashMap<String, String> = HashMap::new();
        for cin in &cf.inputs {
            let b = a.bindings.iter().find(|b| b.input == cin.name.text).unwrap();
            // The same reading `check` makes: a word that names nothing in the applying rule
            // is a literal — `true`, `false`, or a value of the callee's enum. The partial
            // evaluation below already knew what to do with one; nothing could produce it
            // (DESIGN §15.90).
            let cty = cc.ty_of(&cin.name.text).map(|t| match t {
                Ty::Opt(i) => *i,
                t => t,
            });
            let word_lit = |w: &str| match &cty {
                Some(Ty::Bool) => w == crate::kw::TRUE || w == crate::kw::FALSE,
                Some(Ty::Enum(ke)) => {
                    cc.enums.iter().any(|(e, vs)| e == ke && vs.contains(&w.to_string()))
                }
                _ => false,
            };
            subs.insert(cin.name.text.clone(), match &b.value {
                BindValue::Name(n) if !declares(f, n) && word_lit(n) => {
                    Sub::Lit(Lit::Word(n.clone()))
                }
                BindValue::Name(n) => Sub::Name(n.clone()),
                BindValue::Lit(l) => Sub::Lit(l.clone()),
            });
            if let Some(Ty::Enum(en)) = cc.ty_of(&cin.name.text).map(|t| match t {
                Ty::Opt(i) => *i,
                t => t,
            }) {
                input_enum.insert(cin.name.text.clone(), en.clone());
                if let BindValue::Name(caller) = &b.value {
                    // This rule's values for the callee's: `with` first, then the same spelling.
                    let callee_vals = cc.enums.get(&en).cloned().unwrap_or_default();
                    let caller_vals = caller_enum_values(f, caller);
                    let mut mp: HashMap<String, Vec<String>> = HashMap::new();
                    for (from, to) in &b.map {
                        mp.entry(to.clone()).or_default().push(from.clone());
                    }
                    for v in &callee_vals {
                        // The absent value read as one of the callee's (`with none -> …`, §15.255) adds
                        // to the values that stand for it, and takes none of them away.
                        let taken = b.map.iter().any(|(from, to)| to == v && from != crate::kw::NONE);
                        if !taken && caller_vals.as_ref().is_none_or(|vs| vs.contains(v)) && b.map.iter().all(|(from, _)| from != v) {
                            mp.entry(v.clone()).or_default().push(v.clone());
                        }
                    }
                    // This rule's values in their order: as its enum declares them, or, for a name
                    // whose enum this reading does not find, as they stand for the callee's in the
                    // callee's order. A rule that passes E042 has each stand for one.
                    let ordered = caller_vals.unwrap_or_else(|| {
                        let mut out: Vec<String> = Vec::new();
                        for v in &callee_vals {
                            for w in mp.get(v).into_iter().flatten() {
                                if w != crate::kw::NONE && !out.contains(w) {
                                    out.push(w.clone());
                                }
                            }
                        }
                        out
                    });
                    own.insert(cin.name.text.clone(), ordered);
                    maps.insert(cin.name.text.clone(), mp);
                }
            }
        }
        for (callee_field, ours) in &field_subs {
            subs.insert(callee_field.clone(), Sub::Name(ours.clone()));
        }
        let rn = Renamer {
            apply: &an,
            alias: cal_alias.clone(),
            at: a.span.clone(),
            subs,
            maps,
            groups: cc.groups.iter().map(|(g, (_, ms))| (g.clone(), ms.clone())).collect(),
            own,
            input_enum,
            enum_names: enum_names.clone(),
        };

        // The inlined items.
        let mut items: Vec<Item> = Vec::new();
        for it in &cf.items {
            match it {
                Item::Derived(d) => items.push(Item::Derived(DerivedDecl {
                    name: rn.name(&d.name),
                    ty: rn.ty(&d.ty),
                    expr: rn.expr(&d.expr),
                    range: d.range.clone(),
                    span: rn.sp(),
                    cite: rn.cite(&d.cite),
                })),
                Item::Define(d) => items.push(Item::Define(DefineDecl {
                    name: rn.name(&d.name),
                    ty: rn.ty(&d.ty),
                    expr: rn.expr(&d.expr),
                    span: rn.sp(),
                    cite: rn.cite(&d.cite),
                })),
                // A summary or a fold of a callee that walks a sequence walks this rule's (§15.230),
                // under this apply's name.
                Item::Agg(d) => items.push(Item::Agg(AggDecl {
                    name: rn.name(&d.name),
                    over: seq_subs.iter().find(|(c, _)| *c == d.over).map(|(_, m)| m.clone()).unwrap_or_else(|| d.over.clone()),
                    column: Name { text: rn.word(&d.column.text), span: rn.sp(), ..d.column.clone() },
                    filter: d.filter.as_ref().map(|(c, v)| (Name { text: rn.word(&c.text), span: rn.sp(), ..c.clone() }, v.clone())),
                    span: rn.sp(),
                    ..d.clone()
                })),
                Item::Fold(d) => {
                    // A fold that names no answer answers the callee's first output, which comes in
                    // as `<apply>:<output>` here.
                    let answer = match &d.answer {
                        Some(x) => FoldAnswer { name: rn.name(&x.name), ty: x.ty.clone().or_else(|| cf.outputs.iter().find(|o| o.name.text == x.name.text).map(|o| o.ty.clone())).map(|t| rn.ty(&t)) },
                        None => {
                            let o = &cf.outputs[0];
                            FoldAnswer { name: rn.name(&o.name), ty: Some(rn.ty(&o.ty)) }
                        }
                    };
                    items.push(Item::Fold(FoldDecl {
                        verdict: rn.word(&d.verdict),
                        over: seq_subs.iter().find(|(c, _)| *c == d.over).map(|(_, m)| m.clone()).unwrap_or_else(|| d.over.clone()),
                        answer: Some(answer),
                        arms: d
                            .arms
                            .iter()
                            .map(|(v, arm, _)| {
                                let arm = match arm {
                                    Arm::Stop(Some(x)) => Arm::Stop(Some(rn.expr(x))),
                                    Arm::Take { expr, unique } => Arm::Take { expr: rn.expr(expr), unique: *unique },
                                    Arm::KeepMax { expr, key } => Arm::KeepMax { expr: rn.expr(expr), key: rn.expr(key) },
                                    Arm::TakeWeighted { expr, weight } => Arm::TakeWeighted { expr: rn.expr(expr), weight: rn.expr(weight) },
                                    other => other.clone(),
                                };
                                (Name { span: rn.sp(), ..v.clone() }, arm, rn.sp())
                            })
                            .collect(),
                        empty: d.empty.as_ref().map(|x| rn.expr(x)),
                        exhausted: d.exhausted.as_ref().map(|x| rn.expr(x)),
                        // The drawn value of a pick by weight (§15.226) is the callee's input, read
                        // as this rule binds it.
                        draw: d.draw.as_ref().map(|n| Name { text: rn.word(&n.text), ascii: None, span: rn.sp() }),
                        span: rn.sp(),
                    }));
                }
                Item::Table(t) => {
                    let tn = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                    if dropped_tables.contains(&tn) {
                        continue;
                    }
                    match inline_table(&rn, &cc, t, &dropped_rows, &an, &cf.outputs) {
                        Some(nt) => items.push(Item::Table(nt)),
                        // Every row went with the bindings: the literal bound to a column
                        // matched none, or the enum values the rows name stand for nothing here.
                        None => out.push(
                            Diag::warning("W118", tr!("{} {} の行は、この準用ではどれも当たりません", "No row of {} {} is reached in this apply", if t.clause { tr!("節", "clause") } else { tr!("表", "table") }, rn.text(&tn)))
                                .at(at(a.span.line))
                                .table(rn.text(&tn))
                                .mark(a.span.clone(), "")
                                .note(tr!(
                                    "読み替えた値では、この表のどの行にも当たりません。元の規則が自分の入力の範囲で完全なことは変わりません。",
                                    "What is bound matches no row of this table. The callee is still complete over its own inputs."
                                ))
                                .note(tr!("この準用に要らない表なら、`{} {}` で外せます。", "If this apply does not need the table, `{} {}` leaves it out.", crate::kw::EXCEPT, tn)),
                        ),
                    }
                }
            }
        }
        // A derived value or definition of the callee that nothing uses any more — its only
        // reader was a definition an `except` left out — would only draw a W111 pointing at
        // the `apply` line. It goes, unless this rule reads it as `<apply>:<name>` (§15.230).
        let mut roots: Vec<String> = cf.outputs.iter().map(|o| rn.text(&o.name.text)).collect();
        let prefix = format!("{an}:");
        for it in &f.items {
            roots.extend(crate::types::item_reads(it).into_iter().filter(|n| n.starts_with(&prefix)));
        }
        if let Some(r) = &f.result {
            roots.extend(expr_names(&r.expr).into_iter().filter(|n| n.starts_with(&prefix)));
        }
        for o in &f.opens {
            roots.extend(o.terms.iter().map(|t| t.name.text.clone()).filter(|n| n.starts_with(&prefix)));
        }
        prune(&mut items, &roots, cf.result.as_ref().map(|r| rn.expr(&r.expr)));
        // The callee's outputs: its first one may be a `result` expression, the rest are
        // bindings of their own names; each becomes a definition here, under the name this
        // rule gives it, rounded as the callee declared.
        for (oi, od) in cf.outputs.iter().enumerate() {
            let inner = rn.text(&od.name.text);
            if let (Some(r), 0) = (&cf.result, oi) {
                items.push(Item::Define(DefineDecl {
                    name: Name { text: inner.clone(), ascii: Some(format!("{}_{}", rn.alias, od.name.ascii.clone().unwrap_or_else(|| od.name.text.clone()))), span: rn.sp() },
                    ty: rn.ty(&od.ty),
                    expr: rn.expr(&r.expr),
                    span: rn.sp(),
                    cite: None,
                }));
            }
            let target: Name = match a.outputs.iter().find(|ob| ob.output == od.name.text) {
                Some(ob) => ob.name.clone(),
                None => Name { text: od.name.text.clone(), ascii: od.name.ascii.clone(), span: a.span.clone() },
            };
            let taken = f.inputs.iter().any(|i| i.name.text == target.text)
                || f.items.iter().any(|it| match it {
                    Item::Derived(d) => d.name.text == target.text,
                    Item::Define(d) => d.name.text == target.text,
                    Item::Agg(d) => d.name.text == target.text,
                    Item::Fold(d) => d.answer.as_ref().is_some_and(|a| a.name.text == target.text),
                    Item::Table(t) => t.name.as_ref().is_some_and(|n| n.text == target.text) || t.outputs.iter().any(|o| o.name.text == target.text),
                });
            if taken {
                out.push(
                    Diag::error("E041", tr!("元の規則の出力 `{}` に付けた名前 `{}` は、この規則に既にあります", "The name `{}` for the callee output `{}` is already declared in this rule", od.name.text, target.text))
                        .at(at(a.span.line))
                        .mark(a.span.clone(), "")
                        .note(tr!("`{} -> <別の名前>` で改名してください。", "Rename it with `{} -> <another name>`.", od.name.text)),
                );
                continue;
            }
            // An optional output with a rounding — every optional number has one (E104) — would be
            // a definition that rounds a value that may be absent, which no expression may hold
            // (§2.1, §15.249). One with none is a definition that names the column, and passes the
            // absent value on as it is.
            if od.ty.optional && od.rounding.is_some() {
                out.push(e044(
                    tr!("元の規則の出力 `{}` は値が無いことがあり、丸めを宣言しています", "the callee's output `{}` may have no value and declares a rounding", od.name.text),
                    tr!(
                        "準用した出力は、元の規則の丸めを掛けた定義になります。値の無いことのある値は式に書けないので、丸めを掛けられません。",
                        "An applied output becomes a definition of this rule, rounded as the callee declares, and a value that may be absent cannot be rounded in an expression."
                    ),
                ));
                continue;
            }
            let value = Expr::Name(inner.clone(), rn.sp());
            let expr = match &od.rounding {
                Some(rd) => Expr::Call(rd.mode.clone(), vec![value, Expr::Lit(Lit::Num(rd.grid.clone()), rn.sp())], rn.sp()),
                None => value,
            };
            // An output of an enum the callee declares, read in this rule's values (§15.230): a table
            // of the callee's values and what each is here, a value spelled the same on both sides
            // standing for itself.
            let mapped = enum_names.iter().find(|(c, _)| *c == od.ty.base).map(|(c, _)| c.clone());
            let ob = a.outputs.iter().find(|ob| ob.output == od.name.text);
            if let (Some(ce), Some(ob)) = (mapped, ob.filter(|ob| !ob.map.is_empty())) {
                let callee_vals = cc.enums.get(&ce).cloned().unwrap_or_default();
                let mut rows_map: Vec<(String, String)> = Vec::new();
                let mut missing: Vec<String> = Vec::new();
                for v in &callee_vals {
                    match ob.map.iter().find(|(from, _)| from == v) {
                        Some((_, to)) => rows_map.push((v.clone(), to.clone())),
                        None => missing.push(v.clone()),
                    }
                }
                let targets: Vec<&String> = ob.map.iter().map(|(_, to)| to).collect();
                let ours = f.enums.iter().find(|e| targets.iter().all(|t| e.values.iter().any(|v| v.text == **t))).map(|e| e.name.clone());
                for v in missing.clone() {
                    if ours.as_ref().is_some_and(|o| f.enums.iter().any(|e| e.name.text == o.text && e.values.iter().any(|x| x.text == v))) {
                        rows_map.push((v.clone(), v.clone()));
                        missing.retain(|x| *x != v);
                    }
                }
                let bad_from: Vec<&String> = ob.map.iter().map(|(from, _)| from).filter(|from| !callee_vals.contains(from)).collect();
                if ours.is_none() || !missing.is_empty() || !bad_from.is_empty() {
                    let why = if ours.is_none() {
                        tr!("`with` の右の値を全部持つ列挙が、この規則にありません", "no enum of this rule has every value on the right of `with`")
                    } else if !bad_from.is_empty() {
                        tr!("{} は元の規則の {ce} の値ではありません", "{} is no value of the callee's {ce}", bad_from.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(", "))
                    } else {
                        tr!("元の規則の値 {} にあたる値がありません", "the callee's values {} stand for nothing here", missing.join(", "))
                    };
                    out.push(bad_map(&at(ob.span.line), &ob.span, why));
                    continue;
                }
                let ours = ours.expect("checked above");
                let mut rows: Vec<Row> = rows_map
                    .iter()
                    .enumerate()
                    .map(|(i, (from, to))| Row {
                        cells: vec![Cell::Lit(Lit::Word(from.clone()))],
                        cell_spans: vec![ob.span.clone()],
                        outs: vec![OutCell::Lit(Lit::Word(to.clone()))],
                        out_spans: vec![ob.span.clone()],
                        span: ob.span.clone(),
                        index: i + 1,
                        cite: None,
                        label: None,
                        origin: None,
                    })
                    .collect();
                // An optional output's absent value is absent here too (§15.249).
                if od.ty.optional {
                    rows.push(Row {
                        cells: vec![Cell::Nothing],
                        cell_spans: vec![ob.span.clone()],
                        outs: vec![OutCell::Nothing],
                        out_spans: vec![ob.span.clone()],
                        span: ob.span.clone(),
                        index: rows.len() + 1,
                        cite: None,
                        label: None,
                        origin: None,
                    });
                }
                a.defines.push(target.text.clone());
                items.push(Item::Table(Table {
                    name: Some(Name { text: format!("{an}:{}_as_{}", od.name.text, target.text), ascii: None, span: ob.span.clone() }),
                    policy: Policy::Unique,
                    inputs: vec![(inner.clone(), ob.span.clone())],
                    outputs: vec![OutCol { name: Name { text: target.text.clone(), ascii: target.ascii.clone(), span: ob.span.clone() }, ty: Some(TypeRef { base: ours.text.clone(), args: Vec::new(), optional: od.ty.optional, span: ob.span.clone() }), span: ob.span.clone() }],
                    rows,
                    span: ob.span.clone(),
                    overrides: Vec::new(),
                    clause: false,
                    cite: None,
                    applied: Some(an.clone()),
                    open: None,
                    contributed: None,
                }));
                continue;
            }
            // The output's own alias is the name this rule declares it under, if it is an
            // output here; else the alias written on the `->` name, if any; else none, and
            // the name itself is the identifier (§1.3). It used to fall back to the callee's
            // alias with the apply's prefix — which is exactly the identifier the callee's
            // own output column gets when it carries the output's name, the usual shape of a
            // one-table rule — so TypeScript, JavaScript, Go and Swift saw one identifier
            // declared twice (§15.71).
            let ascii = f.outputs.iter().find(|o| o.name.text == target.text).and_then(|o| o.name.ascii.clone()).or_else(|| target.ascii.clone());
            a.defines.push(target.text.clone());
            items.push(Item::Define(DefineDecl {
                name: Name { text: target.text.clone(), ascii, span: target.span.clone() },
                ty: rn.ty(&od.ty),
                expr,
                span: a.span.clone(),
                cite: None,
            }));
        }
        // The callee's sources, under this apply's name, still resolving beside the callee.
        for s in &cf.sources {
            f.sources.push(SourceDecl {
                name: rn.name(&s.name),
                kind: s.kind.clone(),
                pins: s.pins.iter().map(|p| Pin { span: rn.sp(), ..p.clone() }).collect(),
                span: rn.sp(),
                base: Some(cp.clone()),
            });
        }
        // The divisions the callee's cells name need the enum here too — unless this rule
        // imports the same country already, under the other name (`std/都道府県` and
        // `std/jp/prefectures`): its cells are then spelled this rule's way (§15.182).
        for (p, sp) in &cf.imports {
            let country = crate::prelude::lookup(p).map(|i| i.ns.country);
            let held = f.imports.iter().any(|(q, _)| q == p || (country.is_some() && crate::prelude::lookup(q).map(|i| i.ns.country) == country));
            if !held {
                f.imports.push((p.clone(), sp.clone()));
            }
        }
        // The callee's enums that what came in names as a type come in as `<apply>:<enum>`; one
        // only an input of the callee has is this rule's, through the binding.
        let mut used: Vec<String> = Vec::new();
        for it in &items {
            match it {
                Item::Derived(d) => used.push(d.ty.base.clone()),
                Item::Define(d) => used.push(d.ty.base.clone()),
                Item::Table(t) => used.extend(t.outputs.iter().filter_map(|o| o.ty.as_ref().map(|x| x.base.clone()))),
                Item::Fold(d) => used.extend(d.answer.iter().filter_map(|x| x.ty.as_ref().map(|t| t.base.clone()))),
                Item::Agg(_) => {}
            }
        }
        for e in &cf.enums {
            let copy = format!("{an}:{}", e.name.text);
            if used.contains(&copy) && !f.enums.iter().any(|m| m.name.text == copy) {
                f.enums.push(EnumDecl {
                    name: Name { text: copy.clone(), ascii: Some(format!("{cal_alias}_{}", e.name.ascii.clone().unwrap_or_else(|| e.name.text.clone()))), span: a.span.clone() },
                    from: None,
                    added: Vec::new(),
                    span: a.span.clone(),
                    ..e.clone()
                });
            }
        }
        let n = items.len();
        let at_idx = a.at.min(f.items.len());
        f.items.splice(at_idx..at_idx, items);
        // An open value written after the apply goes after what it brought in (§15.227).
        for o in f.opens.iter_mut() {
            if o.at > at_idx || (o.at == at_idx && o.span.line > a.span.line) {
                o.at += n;
            }
        }
        a.count = n;
        reorder_after_overrides(f, at_idx, n, &an);
        a.callee = Some(Callee {
            fields: field_pairs.clone(),
            rule: cf.name.clone(),
            inputs: cf
                .inputs
                .iter()
                .map(|i| CalleeInput {
                    name: i.name.text.clone(),
                    ty: cc.ty_of(&i.name.text).unwrap_or(Ty::Unknown),
                    range: i.range.clone(),
                    enum_range: cc.enum_ranges.get(&i.name.text).cloned(),
                })
                .collect(),
            constraints: cf.constraints.clone(),
            // Every enum the callee's inputs can take, an imported one too (§15.182): a binding
            // between two inputs of the built-in divisions is checked against these values.
            enums: cc.enum_order.iter().filter_map(|e| cc.enums.get(e).map(|vs| (e.clone(), vs.clone()))).collect(),
        });
        // The contributions merged into the callee are part of what came in (§15.237): the rule's
        // check names them, and so do the head of its generated code and `rulec api`.
        for m in &cf.contributions {
            let via = match &m.via {
                Some(inner) => format!("{an}:{inner}"),
                None => an.clone(),
            };
            f.contributions.push(Merged { via: Some(via), inputs: Vec::new(), ..m.clone() });
        }
        shift += n;
        done.push(a);
    }
    f.applies = done;
    out
}

/// A definition of this rule that takes precedence over one the callee brought in (`overrides
/// 呼び出し:表`) joins that table's definition set, and the set is decided only at its last
/// member. Whatever the callee computed from that output — its own defines, the tables
/// reading it, the definitions made for its outputs — would read the output too early where
/// the expansion put it, so those items move to just after the overriding definition.
fn reorder_after_overrides(f: &mut RuleFile, at: usize, n: usize, _an: &str) {
    // The outputs the inlined tables define, each with the place of the last definition of it in
    // the whole rule: a clause of this rule that takes precedence over one of them, or over such
    // a clause in turn, is a member of its set too (§15.230), and the set is decided there.
    let outs: Vec<String> = f.items[at..(at + n).min(f.items.len())]
        .iter()
        .filter_map(|it| if let Item::Table(t) = it { Some(t.outputs.iter().map(|o| o.name.text.clone()).collect::<Vec<_>>()) } else { None })
        .flatten()
        .collect();
    let mut end = (at + n).min(f.items.len());
    for o in outs {
        let defines = |it: &Item| matches!(it, Item::Table(t) if t.outputs.iter().any(|x| x.name.text == o));
        let Some(last) = f.items.iter().rposition(defines) else { continue };
        if last < end {
            continue;
        }
        // Everything of the expansion that reads the output, or reads something that does, in order.
        let mut moving: Vec<usize> = Vec::new();
        let mut names: Vec<String> = vec![o.clone()];
        for j in at..end {
            if defines(&f.items[j]) {
                continue;
            }
            let reads: Vec<String> = match &f.items[j] {
                Item::Derived(d) => expr_names(&d.expr),
                Item::Define(d) => expr_names(&d.expr),
                Item::Agg(_) | Item::Fold(_) => crate::types::item_reads(&f.items[j]),
                Item::Table(u) => {
                    let mut v: Vec<String> = u.inputs.iter().map(|(c, _)| c.clone()).collect();
                    for r in &u.rows {
                        for x in &r.outs {
                            if let OutCell::Name(w) = x {
                                v.push(w.clone());
                            }
                        }
                    }
                    v
                }
            };
            if reads.iter().any(|r| names.contains(r)) {
                moving.push(j);
                names.extend(crate::types::item_names(&f.items[j]));
            }
        }
        if moving.is_empty() {
            continue;
        }
        let mut taken: Vec<Item> = Vec::new();
        for &j in moving.iter().rev() {
            taken.push(f.items.remove(j));
        }
        taken.reverse();
        let dest = last - moving.len() + 1;
        f.items.splice(dest..dest, taken);
        end -= moving.len();
    }
}

pub(crate) fn expr_names(e: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Name(w, _) => out.push(w.clone()),
            Expr::Lit(..) => {}
            Expr::Bin(a, _, b, _) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Call(_, args, _) => args.iter().for_each(|x| walk(x, out)),
        }
    }
    walk(e, &mut out);
    out
}

/// Drop the inlined `derive`s and `define`s nothing reads: not a table cell, not an expression,
/// not the callee's outputs (`roots`) or its `result`. Repeated until nothing more goes, since
/// a reader that goes frees what it read.
fn prune(items: &mut Vec<Item>, roots: &[String], result: Option<Expr>) {
    fn names_in(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Name(w, _) => out.push(w.clone()),
            Expr::Lit(..) => {}
            Expr::Bin(a, _, b, _) => {
                names_in(a, out);
                names_in(b, out);
            }
            Expr::Call(_, args, _) => args.iter().for_each(|x| names_in(x, out)),
        }
    }
    loop {
        let mut used: Vec<String> = roots.to_vec();
        if let Some(r) = &result {
            names_in(r, &mut used);
        }
        for it in items.iter() {
            match it {
                Item::Derived(d) => names_in(&d.expr, &mut used),
                Item::Define(d) => names_in(&d.expr, &mut used),
                Item::Agg(_) | Item::Fold(_) => used.extend(crate::types::item_reads(it)),
                Item::Table(t) => {
                    used.extend(t.inputs.iter().map(|(n, _)| n.clone()));
                    for r in &t.rows {
                        for o in &r.outs {
                            if let OutCell::Name(w) = o {
                                used.push(w.clone());
                            }
                        }
                    }
                }
            }
        }
        let before = items.len();
        items.retain(|it| match it {
            Item::Derived(d) => used.contains(&d.name.text),
            Item::Define(d) => used.contains(&d.name.text),
            _ => true,
        });
        if items.len() == before {
            return;
        }
    }
}

/// The built-in namespace an input of this rule takes its values from (§15.182).
fn caller_import(f: &RuleFile, name: &str) -> Option<crate::prelude::Import> {
    let base = f.inputs.iter().find(|i| i.name.text == name).map(|i| i.ty.base.clone())?;
    crate::prelude::import_of(f, &base)
}

/// The values of the enum a name of this rule has, when that is knowable before the rule
/// is typed: an input, or an output column of a table declared with its type.
fn caller_enum_values(f: &RuleFile, name: &str) -> Option<Vec<String>> {
    let base = f
        .inputs
        .iter()
        .find(|i| i.name.text == name)
        .map(|i| i.ty.base.clone())
        .or_else(|| {
            f.items.iter().find_map(|it| match it {
                Item::Table(t) => t.outputs.iter().find(|o| o.name.text == name).and_then(|o| o.ty.as_ref().map(|t| t.base.clone())),
                _ => None,
            })
        })?;
    if let Some(im) = crate::prelude::import_of(f, &base) {
        return Some(im.values());
    }
    f.enums.iter().find(|e| e.name.text == base).map(|e| e.values.iter().map(|v| v.text.clone()).collect())
}

/// One callee table (or clause), with the bound inputs substituted: a column bound to a
/// name is renamed, one bound to a literal is evaluated away (rows the literal does not
/// match are dropped), an enum column's cells are spelled in this rule's values, and groups
/// are written out. `None` when every row is gone.
fn inline_table(rn: &Renamer, cc: &Checked, t: &Table, dropped_rows: &[(String, String)], an: &str, callee_outputs: &[OutDecl]) -> Option<Table> {
    let tn = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    // Which columns survive, and what each becomes.
    let mut cols: Vec<(String, Span)> = Vec::new();
    for (name, _) in &t.inputs {
        match rn.subs.get(name) {
            Some(Sub::Name(n)) => cols.push((n.clone(), rn.sp())),
            Some(Sub::Lit(_)) => {}
            None => cols.push((rn.text(name), rn.sp())),
        }
    }
    let mut rows: Vec<Row> = Vec::new();
    'rows: for row in &t.rows {
        if let Some(l) = &row.label {
            if dropped_rows.iter().any(|(dt, dl)| *dt == tn && *dl == l.text) {
                continue;
            }
        }
        let mut cells: Vec<Cell> = Vec::new();
        let mut cell_spans: Vec<Span> = Vec::new();
        for (ci, (name, _)) in t.inputs.iter().enumerate() {
            let cell = row.cells.get(ci).cloned().unwrap_or(Cell::DontCare);
            match rn.subs.get(name) {
                Some(Sub::Lit(l)) => {
                    // Partial evaluation: the column is a constant here.
                    let ty = cc.ty_of(name).unwrap_or(Ty::Unknown);
                    let Some(v) = crate::eval::lit_to_val(l, &ty) else { continue 'rows };
                    if !crate::eval::cell_matches(cc, &cell, &v, &ty) {
                        continue 'rows;
                    }
                }
                _ => {
                    let cell = if rn.input_enum.contains_key(name) || cc.groups.keys().any(|g| cell_names(&cell).contains(g)) {
                        match &cell {
                            // A comparison on the callee's ordered enum is the values it covers in
                            // the callee's order; this rule's enum may order them otherwise, so
                            // what is carried over is the values, mapped (§15.210).
                            Cell::Lit(Lit::Word(_)) | Cell::Set(_) | Cell::Ranked { .. } => {
                                let ls: Vec<Lit> = match &cell {
                                    Cell::Lit(l) => vec![l.clone()],
                                    Cell::Set(ls) => ls.clone(),
                                    Cell::Ranked { values, .. } => values.iter().map(|v| Lit::Word(v.clone())).collect(),
                                    _ => unreachable!(),
                                };
                                let Some(cell) = rn.cell_of_words(name, rn.map_words(name, &ls)) else { continue 'rows };
                                cell
                            }
                            Cell::Not(ls) => {
                                let Some(cell) = rn.not_cell_of_words(name, rn.map_words(name, ls)) else { continue 'rows };
                                cell
                            }
                            other => other.clone(),
                        }
                    } else {
                        cell
                    };
                    cells.push(cell);
                    cell_spans.push(rn.sp());
                }
            }
        }
        let outs: Vec<OutCell> = row
            .outs
            .iter()
            .map(|o| match o {
                OutCell::Name(w) => match rn.subs.get(w) {
                    Some(Sub::Name(n)) => OutCell::Name(n.clone()),
                    Some(Sub::Lit(l)) => OutCell::Lit(l.clone()),
                    None if w == crate::kw::TRUE || w == crate::kw::FALSE => OutCell::Name(w.clone()),
                    None if cc.syms.contains_key(w) => OutCell::Name(rn.text(w)),
                    None => OutCell::Name(w.clone()),
                },
                OutCell::Lit(l) => OutCell::Lit(l.clone()),
                OutCell::Nothing => OutCell::Nothing,
            })
            .collect();
        rows.push(Row {
            cells,
            cell_spans,
            outs,
            out_spans: row.out_spans.iter().map(|_| rn.sp()).collect(),
            span: rn.sp(),
            index: row.index,
            label: row.label.as_ref().map(|l| Name { span: rn.sp(), ..l.clone() }),
            origin: None,
            cite: rn.cite(&row.cite),
        });
    }
    if rows.is_empty() {
        return None;
    }
    Some(Table {
        name: t.name.as_ref().map(|n| rn.name(n)),
        policy: t.policy,
        inputs: cols,
        // An output column that names one of the callee's declared outputs took its type from
        // that declaration; here the declaration is gone, so the column carries the type.
        outputs: t
            .outputs
            .iter()
            .map(|o| OutCol {
                name: rn.name(&o.name),
                ty: o.ty.clone().or_else(|| callee_outputs.iter().find(|od| od.name.text == o.name.text).map(|od| od.ty.clone())).map(|t| rn.ty(&t)),
                span: rn.sp(),
            })
            .collect(),
        rows,
        span: rn.sp(),
        overrides: t
            .overrides
            .iter()
            .map(|r| OverrideRef { table: rn.text(&r.table), row: r.row.clone(), span: rn.sp() })
            .collect(),
        clause: t.clause,
        cite: rn.cite(&t.cite),
        applied: Some(an.to_string()),
        open: None,
        contributed: None,
    })
}

fn cell_names(c: &Cell) -> Vec<String> {
    let ls: Vec<&Lit> = match c {
        Cell::Lit(l) => vec![l],
        Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
        Cell::Ranked { values, .. } => return values.clone(),
        _ => vec![],
    };
    ls.into_iter().filter_map(|l| if let Lit::Word(w) = l { Some(w.clone()) } else { None }).collect()
}

/// After this rule is typed: the bindings agree in type (E042), every bound value stays
/// inside the callee's declared range and the callee's constraints follow from this rule's
/// declarations (E043). Nothing here has a `fix.text`: whether to narrow this rule's range or
/// to define the region in a clause of its own is a business decision.
pub fn check(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for a in &f.applies {
        let Some(cal) = &a.callee else { continue };
        let an = &a.name.text;
        let at = |line: usize| tr!("{path}:{line} 準用 {an}", "{path}:{line} apply {an}");
        // Interval of what a binding passes.
        let interval = |b: &Binding| -> Option<(Option<Rat>, Option<Rat>)> {
            match &b.value {
                BindValue::Name(n) => c.ranges.get(n).cloned(),
                BindValue::Lit(Lit::Num(num)) => {
                    let ty = c.ty_of(&b.input).or_else(|| callee_ty(cal, &b.input))?;
                    let v = crate::types::lit_value_in_pub(num, &ty)?;
                    Some((Some(v), Some(v)))
                }
                BindValue::Lit(Lit::Date(y, m, d)) => {
                    let v = crate::types::date_ord(*y, *m, *d);
                    Some((Some(v), Some(v)))
                }
                _ => None,
            }
        };
        let fmt = |r: Rat, ty: &Ty| crate::coverage::show_rat(r, ty);
        for cin in &cal.inputs {
            let Some(b) = a.bindings.iter().find(|b| b.input == cin.name) else { continue };
            let cty = cin.ty.clone();
            // Types. A word that names nothing in this rule is read as a literal — `true`,
            // `false`, or a value of the callee's enum — because that is what a provision
            // applied mutatis mutandis does: 「『A』とあるのは『B』と読み替える」. The parser
            // has no types and cannot tell the two apart, so the choice is made here. The
            // table below has always had an arm for `Lit::Word`, and nothing could reach it
            // (DESIGN §15.90).
            let as_lit = |w: &str| match &cty {
                Ty::Bool => w == crate::kw::TRUE || w == crate::kw::FALSE,
                Ty::Enum(ke) => cal.enums.iter().any(|(e, vs)| e == ke && vs.contains(&w.to_string())),
                _ => false,
            };
            let value = match &b.value {
                BindValue::Name(n) if c.ty_of(n).is_none() && as_lit(n) => {
                    BindValue::Lit(Lit::Word(n.clone()))
                }
                other => other.clone(),
            };
            match &value {
                BindValue::Name(n) => match c.ty_of(n) {
                    None => out.push(
                        Diag::error("E041", tr!("`{n}` という名前はこの規則にありません", "There is no name `{n}` in this rule"))
                            .at(at(b.span.line))
                            .mark(b.span.clone(), "")
                            .note(tr!("読み替え先に書けるのは、この規則の入力・導出・定義・表や節の出力か、リテラルです。", "A binding takes an input, a derived value, a definition, a table or clause output of this rule, or a literal.")),
                    ),
                    Some(ty) => {
                        let bare = |t: Ty| match t {
                            Ty::Opt(i) => *i,
                            t => t,
                        };
                        let (ty, cty) = (bare(ty), bare(cty.clone()));
                        match (&ty, &cty) {
                            (Ty::Enum(ce), Ty::Enum(ke)) => {
                                let caller_vals = c.enums.get(ce).cloned().unwrap_or_default();
                                let callee_vals = cal.enums.iter().find(|(e, _)| e == ke).map(|(_, v)| v.clone()).unwrap_or_default();
                                for (from, to) in &b.map {
                                    // `with none -> <value>` reads the absent value as one of the callee's
                                    // (§15.255): only where this rule's value may be absent and the callee's
                                    // input takes no absent value of its own.
                                    if from == crate::kw::NONE {
                                        if !matches!(c.ty_of(n), Some(Ty::Opt(_))) {
                                            out.push(bad_map(&at(b.span.line), &b.span, tr!("`{n}` は {ce} で、値の無いことがありません", "`{n}` is {ce}, which is never absent")));
                                        } else if matches!(cin.ty, Ty::Opt(_)) {
                                            out.push(bad_map(
                                                &at(b.span.line),
                                                &b.span,
                                                tr!(
                                                    "元の規則の `{}` は値の無いことをそのまま受け付けるので、`with` で読み替えません。`with` から `none` を外してください。",
                                                    "The callee's `{}` takes the absent value itself, which `with` does not read as another: leave `none` out of `with`.",
                                                    cin.name
                                                ),
                                            ));
                                        }
                                    } else if !caller_vals.contains(from) {
                                        out.push(bad_map(&at(b.span.line), &b.span, tr!("`{from}` は {ce} の値ではありません", "`{from}` is not a value of {ce}")));
                                    }
                                    if !callee_vals.contains(to) {
                                        out.push(bad_map(&at(b.span.line), &b.span, tr!("`{to}` は元の規則の {ke} の値ではありません", "`{to}` is not a value of the callee's {ke}")));
                                    }
                                }
                                let unmapped: Vec<&String> = caller_vals
                                    .iter()
                                    .filter(|v| !b.map.iter().any(|(from, _)| from == *v) && !callee_vals.contains(v))
                                    .collect();
                                if !unmapped.is_empty() {
                                    out.push(bad_map(
                                        &at(b.span.line),
                                        &b.span,
                                        tr!(
                                            "{ce} の値 {} にあたる元の規則の値がありません。`{} <この規則の値> -> <元の規則の値>` で全部の値を対応づけてください（同じ綴りの値は書かなくて構いません）。",
                                            "The values {} of {ce} stand for no value of the callee. Map every value with `{} <value> -> <value>` (a value spelled the same on both sides needs no entry).",
                                            unmapped.iter().map(|v| format!("`{v}`")).collect::<Vec<_>>().join(", "),
                                            crate::kw::WITH
                                        ),
                                    ));
                                }
                            }
                            (Ty::Enum(_), _) | (_, Ty::Enum(_)) => out.push(bad_map(&at(b.span.line), &b.span, tr!("`{n}` は {ty} で、元の規則の `{}` は {cty} です", "`{n}` is {ty}; the callee's `{}` is {cty}", cin.name))),
                            _ => {
                                if !ty.unifies(&cty) && !cty.unifies(&ty) {
                                    out.push(bad_map(&at(b.span.line), &b.span, tr!("`{n}` は {ty} で、元の規則の `{}` は {cty} です", "`{n}` is {ty}; the callee's `{}` is {cty}", cin.name)));
                                }
                            }
                        }
                    }
                },
                BindValue::Lit(l) => {
                    let ok = match (l, &cty) {
                        (Lit::Num(num), t) => crate::types::lit_value_in_pub(num, t).is_some(),
                        (Lit::Word(w), Ty::Bool) => w == crate::kw::TRUE || w == crate::kw::FALSE,
                        (Lit::Word(w), Ty::Enum(ke)) => cal.enums.iter().any(|(e, vs)| e == ke && vs.contains(w)),
                        (Lit::Date(..), Ty::Date) => true,
                        (Lit::Str(_), Ty::Str) => true,
                        _ => false,
                    };
                    if !ok {
                        out.push(bad_map(&at(b.span.line), &b.span, tr!("このリテラルは元の規則の `{}`（{cty}）の値ではありません", "This literal is not a value of the callee's `{}` ({cty})", cin.name)));
                    }
                }
            }
            // The absent value (§15.255): what this rule passes may be absent where the callee's
            // input is not optional, and `with` does not read it as one of the callee's values.
            if !matches!(cin.ty, Ty::Opt(_))
                && let BindValue::Name(n) = &value
                && !b.map.iter().any(|(from, _)| from == crate::kw::NONE)
                && let Some(src) = absent_from(f, c, n, &mut Vec::new())
            {
                out.push(absent_passed(c, a, cin, b, n, &src, &at(b.span.line)));
            }
            // Range: what this rule can pass has to stay inside what the callee proved over.
            let Some(r) = &cin.range else { continue };
            // The range of an input of an ordered enum (§15.210, §15.252): every value the binding
            // can pass, read as the callee's through `with`, is one the range lets in.
            if let Some(allowed) = &cin.enum_range {
                if let Some(d) = enum_outside(f, c, a, cal, b, &value, cin, allowed, r, &at(b.span.line)) {
                    out.push(d);
                }
                continue;
            }
            let Some((clo, chi)) = bounds_of(r, &cty) else { continue };
            let reach = interval(b);
            let (lo, hi) = match reach {
                Some((lo, hi)) => (lo, hi),
                None => {
                    out.push(
                        Diag::error("E043", tr!("`{}` に渡す値の取りうる範囲が分かりません", "The interval of what is passed to `{}` is not known", cin.name))
                            .at(at(b.span.line))
                            .mark(b.span.clone(), "")
                            .note(tr!("元の規則の `{}` は、range {} の中で検査されています。渡す側にも範囲が要ります。", "The callee's `{}` was checked over range {}; the value passed needs a range.", cin.name, range_text(r))),
                    );
                    continue;
                }
            };
            let below = match (lo, clo) {
                (Some(l), Some(cl)) => l.cmp_to(cl) == std::cmp::Ordering::Less,
                (None, Some(_)) => true,
                _ => false,
            };
            let above = match (hi, chi) {
                (Some(h), Some(ch)) => h.cmp_to(ch) == std::cmp::Ordering::Greater,
                (None, Some(_)) => true,
                _ => false,
            };
            if below || above {
                let witness = if below { lo } else { hi };
                let w = witness.map(|v| fmt(v, &cty)).unwrap_or_else(|| tr!("上限なし", "unbounded"));
                out.push(
                    Diag::error("E043", tr!("`{}` に渡す値が、元の規則の範囲の外に出ます", "A value passed to `{}` leaves the callee's range", cin.name))
                        .at(at(b.span.line))
                        .mark(b.span.clone(), tr!("渡す値の取りうる範囲: {}", "what is passed: {}", interval_text(lo, hi, &cty)))
                        // The arguments follow the Japanese sentence (the input, the callee's file, the input again,
                        // its range), so the English one takes them by position: the range comes before the file.
                        .note(tr!("例: {} = {w} は、`{}` の `{}` の range {} の外です。", "For example {0} = {w} is outside range {3} of `{2}` in `{1}`.", cin.name, a.path, cin.name, range_text(r)))
                        .note(tr!(
                            "元の規則の完全性は、その範囲の中でだけ証明されています。範囲の外の値には定義がありません。渡す側の範囲を狭めるか、はみ出す部分をこの規則の節で定めてください。どちらにするかは業務の判断です。",
                            "The callee's completeness was proved over that range; outside it there is no definition. Narrow the range on this side, or define that region in a clause of this rule. Which is a business decision."
                        )),
                );
            }
        }
        // The fields of a callee that walks a sequence (§15.230): the field of this rule's sequence
        // that stands for each is of its type, and inside the range the callee proved over.
        for (ours, cin) in &cal.fields {
            let Some(ty) = c.ty_of(ours) else { continue };
            let bare = |t: Ty| match t {
                Ty::Opt(i) => *i,
                t => t,
            };
            let (ty, cty) = (bare(ty), bare(cin.ty.clone()));
            if !ty.unifies(&cty) && !cty.unifies(&ty) {
                out.push(bad_map(&at(a.span.line), &a.span, tr!("`{ours}` は {ty} で、元の規則の要素の `{}` は {cty} です", "`{ours}` is {ty}; the callee's field `{}` is {cty}", cin.name)));
                continue;
            }
            // The absent value (§15.255), as for an input: a field of this rule's elements that may
            // be absent, under a field of the callee's that is not optional.
            if !matches!(cin.ty, Ty::Opt(_)) && let Some(Absent::Declared { ty: fty, .. }) = absent_from(f, c, ours, &mut Vec::new()) {
                out.push(
                    Diag::error(
                        "E043",
                        tr!(
                            "フィールド `{ours}` には値の無いことがありますが、元の規則の要素の `{}` は値の無いことを受け付けません",
                            "The field `{ours}` may be absent, and the callee's field `{}` takes no absent value",
                            cin.name
                        ),
                    )
                    .at(at(a.span.line))
                    .mark(a.span.clone(), tr!("`{ours}` は {fty} です", "`{ours}` is {fty}"))
                    .note(tr!(
                        "`{}` の完全性は、要素の `{}` に値があるところでだけ証明されています。値が無いときの答えは、その表にありません。",
                        "The completeness of `{}` was proved where the field `{}` of an element has a value; its tables have no answer for the absent value.",
                        a.path,
                        cin.name
                    ))
                    .note(tr!(
                        "値がいつもあるなら、`{ours}` を {} と宣言してください。値の無いことがあるなら、元の規則の要素の `{}` が値の無いことを受け付けて答えを決める（{cty}? と `none` の行）ことになり、それは元の規則の変更です。",
                        "If it always has a value, declare `{ours}` as {}. If it can be absent, the callee's field `{}` has to take the absent value and answer it ({cty}? and a row for `none`), which is a change to the callee.",
                        fty.present(),
                        cin.name
                    )),
                );
            }
            let Some(r) = &cin.range else { continue };
            let Some((clo, chi)) = bounds_of(r, &cty) else { continue };
            let (lo, hi) = c.ranges.get(ours).copied().unwrap_or((None, None));
            let below = match (lo, clo) {
                (Some(l), Some(cl)) => l.cmp_to(cl) == std::cmp::Ordering::Less,
                (None, Some(_)) => true,
                _ => false,
            };
            let above = match (hi, chi) {
                (Some(h), Some(ch)) => h.cmp_to(ch) == std::cmp::Ordering::Greater,
                (None, Some(_)) => true,
                _ => false,
            };
            if below || above {
                out.push(
                    Diag::error("E043", tr!("フィールド `{ours}` の値が、元の規則の `{}` の範囲の外に出ます", "The field `{ours}` leaves the range of the callee's `{}`", cin.name))
                        .at(at(a.span.line))
                        .mark(a.span.clone(), tr!("`{ours}` の範囲: {}", "the range of `{ours}`: {}", interval_text(lo, hi, &cty)))
                        .note(tr!("元の規則の `{}` は range {} の中で検査されています。", "The callee's `{}` was checked over range {}.", cin.name, range_text(r)))
                        .note(tr!(
                            "この規則のフィールドの範囲を、元の規則の範囲まで狭めてください。",
                            "Narrow this rule's field to the callee's range."
                        )),
                );
            }
        }
        // Constraints of the callee: proved from this rule's constraints or intervals. A side that
        // adds inputs up is the sum of what is passed for them (§15.216).
        for k in &cal.constraints {
            let bind = |n: &str| a.bindings.iter().find(|b| b.input == n);
            let Some(bl) = k.left.iter().map(|n| bind(n)).collect::<Option<Vec<&Binding>>>() else { continue };
            let Some(br) = k.right.iter().map(|n| bind(n)).collect::<Option<Vec<&Binding>>>() else { continue };
            let names = |bs: &[&Binding]| -> Option<Vec<String>> {
                bs.iter()
                    .map(|b| match &b.value {
                        BindValue::Name(x) => Some(x.clone()),
                        _ => None,
                    })
                    .collect()
            };
            let mirrored = |op: CmpOp| match op {
                CmpOp::Le => CmpOp::Ge,
                CmpOp::Ge => CmpOp::Le,
                CmpOp::Lt => CmpOp::Gt,
                CmpOp::Gt => CmpOp::Lt,
            };
            let same = |p: &[String], q: &[String]| {
                let (mut p, mut q) = (p.to_vec(), q.to_vec());
                p.sort();
                q.sort();
                p == q
            };
            let declared = match (names(&bl), names(&br)) {
                (Some(x), Some(y)) => {
                    (x.len() == 1 && x == y && matches!(k.op, CmpOp::Le | CmpOp::Ge))
                        || f.constraints.iter().any(|kc| {
                            (kc.op == k.op && same(&kc.left, &x) && same(&kc.right, &y))
                                || (kc.op == mirrored(k.op) && same(&kc.left, &y) && same(&kc.right, &x))
                        })
                }
                _ => false,
            };
            // Or the intervals decide it: the whole of one side lies on the right side of the
            // whole of the other.
            let total = |bs: &[&Binding]| -> Option<(Option<Rat>, Option<Rat>)> {
                bs.iter().try_fold((Some(Rat::zero()), Some(Rat::zero())), |(lo, hi), b| {
                    let (l, h) = interval(b)?;
                    Some((lo.zip(l).map(|(x, y)| x.add(y)), hi.zip(h).map(|(x, y)| x.add(y))))
                })
            };
            let by_interval = match (total(&bl), total(&br)) {
                (Some((xl, xh)), Some((yl, yh))) => match k.op {
                    CmpOp::Le => xh.zip(yl).is_some_and(|(p, q)| p.cmp_to(q) != std::cmp::Ordering::Greater),
                    CmpOp::Lt => xh.zip(yl).is_some_and(|(p, q)| p.cmp_to(q) == std::cmp::Ordering::Less),
                    CmpOp::Ge => xl.zip(yh).is_some_and(|(p, q)| p.cmp_to(q) != std::cmp::Ordering::Less),
                    CmpOp::Gt => xl.zip(yh).is_some_and(|(p, q)| p.cmp_to(q) == std::cmp::Ordering::Greater),
                },
                _ => false,
            };
            if !declared && !by_interval {
                let shown = |b: &&Binding| match &b.value {
                    BindValue::Name(n) => n.clone(),
                    BindValue::Lit(l) => lit_text(l),
                };
                let side = |bs: &[&Binding]| bs.iter().map(shown).collect::<Vec<_>>().join(" + ");
                let (sl, sr) = (side(&bl), side(&br));
                let (kl, kr) = (crate::ast::Constraint::side_text(&k.left), crate::ast::Constraint::side_text(&k.right));
                out.push(
                    Diag::error("E043", tr!("元の規則の制約 `{}` が、この規則の宣言から導けません", "The callee's constraint `{}` does not follow from this rule's declarations", k.said()))
                        .at(at(a.span.line))
                        .mark(a.span.clone(), "")
                        .note(tr!("元の規則は `{kl}` と `{kr}` にその関係があると宣言していて、完全性はそれを前提に検査されています。この規則が渡すのは `{sl}` と `{sr}` です。", "The callee declares that relation between `{kl}` and `{kr}`, and its completeness was checked believing it. Here `{sl}` and `{sr}` are passed."))
                        .note(tr!("同じ関係を `{} {sl} {} {sr}` として宣言するか、範囲でそれが成り立つようにしてください。", "Declare the same relation as `{} {sl} {} {sr}`, or make the ranges imply it.", crate::kw::CONSTRAINT, k.op.word())),
                );
            }
        }
    }
    out
}

fn bad_map(at: &str, sp: &Span, what: String) -> Diag {
    Diag::error("E042", tr!("読み替えの型が合いません", "A binding does not agree in type"))
        .at(at.to_string())
        .mark(sp.clone(), "")
        .note(what)
}

fn callee_ty(cal: &Callee, input: &str) -> Option<Ty> {
    cal.inputs.iter().find(|i| i.name == input).map(|i| i.ty.clone())
}

/// What a name of this rule can hold, as values of its enum (§15.252), read the way `types::check`
/// reads the interval of a number. `known` are the values the name is read to take: what an input's
/// range lets in (every value of its enum without one), every value of a field of an element (whose
/// enum takes no range, E069), what the rows of the tables that define it write (a value written,
/// or what a name written can hold, narrowed by the row's own cell for it, as `Checked::row_writes`
/// narrows it), what a definition that only names another value names — which is what an output
/// another `apply` brought in is — and what a `max` or a `min` of two of them comes to (§15.256).
/// `assumed` are the values it is taken to have only because a fold or another expression decides
/// it: check does not work out which values of an enum those come to, so it is every value of the
/// enum, the safe side, as a number whose interval is not known; `exprs` names the definitions they
/// come from.
#[derive(Default, Clone)]
struct Held {
    known: Vec<String>,
    assumed: Vec<String>,
    exprs: Vec<String>,
}

fn add_new(list: &mut Vec<String>, vs: impl IntoIterator<Item = String>) {
    for v in vs {
        if !list.contains(&v) {
            list.push(v);
        }
    }
}

/// [`Held`] of the names of one rule, each read once: a table that writes a name another table
/// writes is read through it, and a name already being read on the way there — which a rule that
/// passes its type check does not have — is read as an expression is.
struct Holds<'a> {
    f: &'a RuleFile,
    c: &'a Checked,
    done: HashMap<String, Held>,
    reading: Vec<String>,
}

impl<'a> Holds<'a> {
    fn new(f: &'a RuleFile, c: &'a Checked) -> Holds<'a> {
        Holds { f, c, done: HashMap::new(), reading: Vec::new() }
    }

    fn of(&mut self, name: &str) -> Held {
        if let Some(h) = self.done.get(name) {
            return h.clone();
        }
        if self.reading.iter().any(|n| n == name) {
            return self.computed(name);
        }
        self.reading.push(name.to_string());
        let h = self.read(name);
        self.reading.pop();
        self.done.insert(name.to_string(), h.clone());
        h
    }

    fn every(&self, name: &str) -> Vec<String> {
        self.c
            .ty_of(name)
            .and_then(|t| match t.present() {
                Ty::Enum(e) => self.c.enums.get(e).cloned(),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn computed(&self, name: &str) -> Held {
        Held { known: Vec::new(), assumed: self.every(name), exprs: vec![name.to_string()] }
    }

    fn read(&mut self, name: &str) -> Held {
        let (f, c) = (self.f, self.c);
        if f.inputs.iter().any(|i| i.name.text == name) {
            return Held { known: c.enum_ranges.get(name).cloned().unwrap_or_else(|| self.every(name)), ..Held::default() };
        }
        if f.element_fields().any(|x| x.name.text == name) {
            return Held { known: self.every(name), ..Held::default() };
        }
        let mut out = Held::default();
        let mut tabled = false;
        for it in &f.items {
            let Item::Table(t) = it else { continue };
            let Some(oi) = t.outputs.iter().position(|o| o.name.text == name) else { continue };
            tabled = true;
            for row in &t.rows {
                match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Word(w))) => add_new(&mut out.known, [w.clone()]),
                    Some(OutCell::Name(w)) => match c.ty_of(w) {
                        // No name of that spelling where the cell is: the word is a value.
                        None => add_new(&mut out.known, [w.clone()]),
                        Some(wty) => {
                            let inner = self.of(w);
                            let cell = t.inputs.iter().position(|(n, _)| n == w).and_then(|ci| row.cells.get(ci));
                            let lets_in = |v: &String| cell.is_none_or(|cell| crate::eval::cell_matches(c, cell, &crate::eval::Val::Enum(v.clone()), &wty));
                            add_new(&mut out.known, inner.known.into_iter().filter(|v| lets_in(v)));
                            let assumed: Vec<String> = inner.assumed.into_iter().filter(|v| lets_in(v)).collect();
                            if !assumed.is_empty() {
                                add_new(&mut out.assumed, assumed);
                                add_new(&mut out.exprs, inner.exprs);
                            }
                        }
                    },
                    // `none`, or a literal of no enum: no value of one.
                    _ => {}
                }
            }
        }
        if tabled {
            return out;
        }
        let expr = f.items.iter().find_map(|it| match it {
            Item::Define(d) if d.name.text == name => Some(&d.expr),
            Item::Derived(d) if d.name.text == name => Some(&d.expr),
            _ => None,
        });
        match expr {
            Some(e) => self.of_expr(e, name),
            None => self.computed(name),
        }
    }

    /// What an expression that decides `name` can come to (§15.256): a name what it holds, and `max`
    /// and `min` one of their two arguments — read as a number's
    /// interval is, from the ends of what the arguments hold in the enum's order: `max` comes to
    /// no value before the later of the two first values nor after the later of the two last ones,
    /// and `min` to none before the earlier of the first ones nor after the earlier of the last
    /// ones. Any other expression is any value of the enum, as check does not work it out.
    fn of_expr(&mut self, e: &Expr, name: &str) -> Held {
        let order = self.every(name);
        match e {
            Expr::Name(w, _) if self.c.ty_of(w).is_some() => self.of(w),
            Expr::Call(fun, args, _) if (fun == crate::kw::MAX || fun == crate::kw::MIN) && args.len() == 2 => {
                let (a, b) = (self.of_expr(&args[0], name), self.of_expr(&args[1], name));
                let at = |v: &String| order.iter().position(|x| x == v);
                let ends = |h: &Held| {
                    let ps: Vec<usize> = h.known.iter().chain(&h.assumed).filter_map(at).collect();
                    Some((*ps.iter().min()?, *ps.iter().max()?))
                };
                let (Some((alo, ahi)), Some((blo, bhi))) = (ends(&a), ends(&b)) else { return self.computed(name) };
                let (lo, hi) = if fun == crate::kw::MAX { (alo.max(blo), ahi.max(bhi)) } else { (alo.min(blo), ahi.min(bhi)) };
                let inside = |v: &String| at(v).is_some_and(|p| lo <= p && p <= hi);
                let mut out = Held::default();
                add_new(&mut out.known, a.known.iter().chain(&b.known).filter(|v| inside(v)).cloned());
                let assumed: Vec<String> = a.assumed.iter().chain(&b.assumed).filter(|v| inside(v) && !out.known.contains(v)).cloned().collect();
                if !assumed.is_empty() {
                    add_new(&mut out.assumed, assumed);
                    add_new(&mut out.exprs, a.exprs.into_iter().chain(b.exprs));
                }
                out
            }
            _ => self.computed(name),
        }
    }
}

/// The callee's values a value of this rule stands for through a binding, as the expansion reads
/// it: what `with` maps it to, or else the value spelled the same, unless `with` maps another value
/// to that one. The absent value read as one of the callee's (`with none -> …`, §15.255) takes no
/// value from the one spelled the same.
fn stands_for(b: &Binding, v: &str, callee_vals: &[String]) -> Vec<String> {
    if b.map.iter().any(|(from, _)| from == v) {
        return b.map.iter().filter(|(from, to)| from == v && callee_vals.contains(to)).map(|(_, to)| to.clone()).collect();
    }
    if callee_vals.iter().any(|x| x == v) && !b.map.iter().any(|(from, to)| to == v && from != crate::kw::NONE) {
        return vec![v.to_string()];
    }
    Vec::new()
}

/// Where a name of this rule gets the absent value from (§15.255).
enum Absent {
    /// An input or a field of an element declared `T?`: the caller may leave it out.
    Declared { name: String, ty: Ty },
    /// A row of a table that answers `none`, by its number in the table.
    Row { table: String, row: usize },
}

/// Whether a name of this rule can hold the absent value, and where it gets it: an input or a field
/// of an element declared `T?` can; an output of a table can where a row answers `none` or passes
/// on a name that can; a definition that only names another value can where that one can. An
/// expression never holds it (§2.1), and a name whose type is not optional never does. A name met
/// again on the way is read as one that can, the safe side.
fn absent_from(f: &RuleFile, c: &Checked, name: &str, seen: &mut Vec<String>) -> Option<Absent> {
    let ty = c.ty_of(name)?;
    if !matches!(ty, Ty::Opt(_)) {
        return None;
    }
    if f.inputs.iter().any(|i| i.name.text == name) || f.element_fields().any(|x| x.name.text == name) || seen.iter().any(|n| n == name) {
        return Some(Absent::Declared { name: name.to_string(), ty });
    }
    seen.push(name.to_string());
    let mut tabled = false;
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        let Some(oi) = t.outputs.iter().position(|o| o.name.text == name) else { continue };
        tabled = true;
        let table = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        for (ri, row) in t.rows.iter().enumerate() {
            match row.outs.get(oi) {
                Some(OutCell::Nothing) => return Some(Absent::Row { table, row: ri + 1 }),
                Some(OutCell::Name(w)) if c.ty_of(w).is_some() => {
                    if let Some(a) = absent_from(f, c, w, seen) {
                        return Some(a);
                    }
                }
                _ => {}
            }
        }
    }
    if tabled {
        return None;
    }
    let expr = f.items.iter().find_map(|it| match it {
        Item::Define(d) if d.name.text == name => Some(&d.expr),
        Item::Derived(d) if d.name.text == name => Some(&d.expr),
        _ => None,
    });
    match expr {
        Some(Expr::Name(w, _)) if c.ty_of(w).is_some() => absent_from(f, c, w, seen),
        // Any other expression: it never comes to the absent value.
        Some(_) => None,
        // A name declared some other way (a fold's answer, what a walk leaves): it may.
        None => Some(Absent::Declared { name: name.to_string(), ty }),
    }
}

/// E043 for a value that may be absent, passed to an input of the callee that is not optional
/// (§15.255). The callee's completeness was proved where the input has a value, and it has no
/// answer for the absent one; E101 used to say so in a table of the callee's, and asked for a row
/// there, which this rule cannot add. What this rule can write is said instead.
fn absent_passed(c: &Checked, a: &ApplyDecl, cin: &CalleeInput, b: &Binding, n: &str, src: &Absent, at: &str) -> Diag {
    let cty = &cin.ty;
    let mark = match src {
        Absent::Declared { name, ty } if name == n => tr!("`{name}` は {ty} です", "`{name}` is {ty}"),
        Absent::Declared { name, ty } => tr!("`{n}` は `{name}`（{ty}）を指します", "`{n}` names `{name}`, which is {ty}"),
        Absent::Row { table, row } => tr!("表 {table} の 行{row} が none を書きます", "row {row} of table {table} answers none"),
    };
    let mut d = Diag::error(
        "E043",
        tr!(
            "`{}` に渡す値には値の無いことがありますが、元の規則の `{}` は値の無いことを受け付けません",
            "What is passed to `{}` may be absent, and the callee's `{}` takes no absent value",
            cin.name,
            cin.name
        ),
    )
    .at(at.to_string())
    .mark(b.span.clone(), mark)
    .note(tr!(
        "`{}` の完全性は、`{}` に値があるところでだけ証明されています。値が無いときの答えは、その表にありません。",
        "The completeness of `{}` was proved where `{}` has a value; its tables have no answer for the absent value.",
        a.path,
        cin.name
    ));
    // What this rule can write: read the absent value as a value of the callee's, give the name a
    // value always, or decide one in a table of this rule that answers `none` too.
    let always = match src {
        Absent::Declared { name, ty } => tr!("値がいつもあるなら、`{name}` を {} と宣言してください。", "If it always has a value, declare `{name}` as {}.", ty.present()),
        Absent::Row { table, row } => tr!("表 {table} の 行{row} が、`none` ではなく値を書くようにしてください。", "Have row {row} of table {table} answer a value rather than `none`."),
    };
    if matches!(cty, Ty::Enum(_)) && matches!(c.ty_of(n).as_ref().map(Ty::present), Some(Ty::Enum(_))) {
        d = d.note(tr!(
            "`{} = {n} with none -> <値>` のように、`with` で、値の無いことを元の規則の値として読めます。{always}`none` にも答える表をこの規則に書いて値を決め、それを渡すこともできます。",
            "`with` reads the absent value as a value of the callee's: `{} = {n} with none -> <value>`. {always} A table of this rule that answers `none` too can also decide the value, and pass that.",
            cin.name
        ));
    } else {
        d = d.note(tr!(
            "{always}値の無いことがあるなら、元の規則の `{}` が値の無いことを受け付けて答えを決める（{cty}? と `none` の行）ことになり、それは元の規則の変更です。",
            "{always} If it can be absent, the callee's `{}` has to take the absent value and answer it ({cty}? and a row for `none`), which is a change to the callee.",
            cin.name
        ));
    }
    d
}

/// E043 for an input of the callee held to a range of its ordered enum (§15.252): the values the
/// binding can pass, read as the callee's through `with`, against the values the range lets in. A
/// value the binding is read to pass that the range does not let in is said as a number outside its
/// range is, with it as the example. Values taken only because an expression decides what is
/// passed are said as a number whose interval is not known is.
#[allow(clippy::too_many_arguments)]
fn enum_outside(f: &RuleFile, c: &Checked, a: &ApplyDecl, cal: &Callee, b: &Binding, value: &BindValue, cin: &CalleeInput, allowed: &[String], r: &Range, at: &str) -> Option<Diag> {
    let callee_vals: Vec<String> = match cin.ty.present() {
        Ty::Enum(ke) => cal.enums.iter().find(|(e, _)| e == ke).map(|(_, vs)| vs.clone()).unwrap_or_default(),
        _ => return None,
    };
    let passed = match value {
        BindValue::Lit(Lit::Word(w)) if callee_vals.contains(w) => Held { known: vec![w.clone()], ..Held::default() },
        BindValue::Name(n) if matches!(c.ty_of(n).as_ref().map(Ty::present), Some(Ty::Enum(_))) => {
            let h = Holds::new(f, c).of(n);
            let read = |vs: Vec<String>| {
                let mut out = Vec::new();
                for v in vs {
                    add_new(&mut out, stands_for(b, &v, &callee_vals));
                }
                out
            };
            Held { known: read(h.known), assumed: read(h.assumed), exprs: h.exprs }
        }
        // A binding of another type, or a literal that is no value of the callee's enum, is E042's.
        _ => return None,
    };
    let outside: Vec<&String> = callee_vals.iter().filter(|v| passed.known.contains(v) && !allowed.contains(v)).collect();
    let unsure: Vec<&String> = callee_vals.iter().filter(|v| passed.assumed.contains(v) && !passed.known.contains(v) && !allowed.contains(v)).collect();
    if let Some(w) = outside.first() {
        let shown: Vec<&str> = callee_vals.iter().filter(|v| passed.known.contains(v) || passed.assumed.contains(v)).map(String::as_str).collect();
        let mark = match shown.as_slice() {
            [one] => tr!("渡す値: {one}", "what is passed: {one}"),
            many => tr!("渡す値の取りうる値: {}", "what is passed: one of {}", many.join(", ")),
        };
        return Some(
            Diag::error("E043", tr!("`{}` に渡す値が、元の規則の範囲の外に出ます", "A value passed to `{}` leaves the callee's range", cin.name))
                .at(at.to_string())
                .mark(b.span.clone(), mark)
                // The arguments in the order of the Japanese sentence, as the number's (§15.179).
                .note(tr!("例: {} = {w} は、`{}` の `{}` の range {} の外です。", "For example {0} = {w} is outside range {3} of `{2}` in `{1}`.", cin.name, a.path, cin.name, range_text(r)))
                .note(tr!(
                    "元の規則の完全性は、その範囲の中でだけ証明されています。範囲の外の値には定義がありません。渡す値を範囲の中の値に限るか、はみ出す値をこの規則の節で定めてください。どちらにするかは業務の判断です。",
                    "The callee's completeness was proved over that range; outside it there is no definition. Narrow what is passed to the values inside it, or define the values outside it in a clause of this rule. Which is a business decision."
                )),
        );
    }
    if unsure.is_empty() {
        return None;
    }
    let exprs = passed.exprs.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(tr!("・", ", ").as_str());
    let values = unsure.iter().map(|v| v.as_str()).collect::<Vec<_>>().join(", ");
    let (is, one) = if passed.exprs.len() > 1 { ("are", "each is read as one") } else { ("is", "it is read as one") };
    Some(
        Diag::error("E043", tr!("`{}` に渡す値の取りうる値が分かりません", "The values of what is passed to `{}` are not known", cin.name))
            .at(at.to_string())
            .mark(b.span.clone(), "")
            .note(tr!(
                "元の規則の `{}` は、range {} の中で検査されています。{exprs} は `fold` の答えか式で決まる値で、check はそれがとる列挙の値を求めないので、{values} にもなりうるとして読みます。",
                "The callee's `{}` was checked over range {}. {exprs} {is} decided by a fold or an expression, and check does not work out which values of an enum those come to, so {one} that can be {values} too.",
                cin.name,
                range_text(r)
            ))
            .note(tr!(
                "範囲の中の値だけを書く表で値を決めて渡すか、同じ range を書いた入力を渡してください。",
                "Decide the value in a table that writes only values inside the range and pass that, or pass an input that declares the same range."
            )),
    )
}

fn bounds_of(r: &Range, ty: &Ty) -> Option<(Option<Rat>, Option<Rat>)> {
    let mut lo = None;
    let mut hi = None;
    for (op, l) in &r.bounds {
        let v = match l {
            Lit::Num(n) => crate::types::lit_value_in_pub(n, ty)?,
            Lit::Date(y, m, d) => crate::types::date_ord(*y, *m, *d),
            _ => return None,
        };
        match op {
            CmpOp::Ge | CmpOp::Gt => lo = Some(v),
            CmpOp::Le | CmpOp::Lt => hi = Some(v),
        }
    }
    Some((lo, hi))
}

fn range_text(r: &Range) -> String {
    r.bounds
        .iter()
        .map(|(op, l)| format!("{}{}", op.word(), lit_text(l)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn interval_text(lo: Option<Rat>, hi: Option<Rat>, ty: &Ty) -> String {
    let f = |v: Option<Rat>| v.map(|v| crate::coverage::show_rat(v, ty)).unwrap_or_else(|| "…".to_string());
    tr!("{} 〜 {}", "{} .. {}", f(lo), f(hi))
}

fn lit_text(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Word(w) => w.clone(),
        Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        Lit::Str(s) => format!("\"{s}\""),
    }
}
