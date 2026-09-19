//! `apply` (DESIGN-draft §5, §15.69): a rule applied with its inputs bound — a provision
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

/// The heading as `rulec source pin` writes it and as the fix offers it.
pub fn header_line(a: &ApplyDecl, hash: &str) -> String {
    let name = match &a.name.ascii {
        Some(al) => format!("{}({al})", a.name.text),
        None => a.name.text.clone(),
    };
    format!("{} {name} = \"{}\" sha256:{hash}", crate::kw::APPLY, a.path)
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
    /// Callee input → the enum it has, when it is an enum.
    input_enum: HashMap<String, String>,
}

impl Renamer<'_> {
    fn text(&self, n: &str) -> String {
        format!("{}:{n}", self.apply)
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
            None if w == crate::kw::TRUE || w == crate::kw::FALSE => Expr::Name(w.to_string(), self.sp()),
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
    fn cite(&self, c: &Option<Cite>) -> Option<Cite> {
        c.as_ref().map(|c| Cite { source: self.text(&c.source), fragments: c.fragments.clone(), span: self.sp() })
    }
}

/// Expand every `apply` of `f` in place. Errors are the callee not being usable (E044), its
/// pin (E040) and the shape of the bindings (E041, E035); what needs this rule's types is
/// left to [`check`].
pub fn expand(f: &mut RuleFile, path: &str) -> Vec<Diag> {
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
        match &a.hash {
            None => out.push(
                Diag::error("E040", tr!("元の規則 `{}` のハッシュが固定されていません", "The digest of the callee `{}` is not pinned", a.path))
                    .at(at(a.span.line))
                    .mark(a.span.clone(), "")
                    .note(tr!("いまの元の規則は sha256:{h} です。この内容で承認するなら、見出しを次のとおりにしてください（`rulec source pin` でも書けます）。", "The callee is sha256:{h} now. To pin it as the one approved, make the heading the following (`rulec source pin` writes it too)."))
                    .fix(crate::diag::FixKind::PinSource, header_line(&a, &h)),
            ),
            Some(p) if *p != h => out.push(
                Diag::error("E040", tr!("元の規則 `{}` が変わっています", "The callee `{}` has changed", a.path))
                    .at(at(a.span.line))
                    .mark(a.span.clone(), tr!("固定: sha256:{p}", "pinned: sha256:{p}"))
                    .note(tr!("いまの元の規則: sha256:{h}", "The callee now: sha256:{h}"))
                    .note(tr!("`rulec diff` でこの規則の答えが何件いくら動くかを見てから、見出しを次のとおり書き換えてハッシュを固定し直してください。", "See with `rulec diff` how many answers of this rule move and by how much, then rewrite the heading as follows to pin the new callee."))
                    .fix(crate::diag::FixKind::PinSource, header_line(&a, &h)),
            ),
            _ => {}
        }
        let src = String::from_utf8_lossy(&bytes).into_owned();
        let cp = cpath.to_string_lossy().into_owned();
        a.callee_src = src.clone();
        a.callee_path = cp.clone();
        // One level: a callee with an `apply` of its own is refused before anything reads
        // through it, so a rule that applies itself by way of another cannot recurse here.
        let parsed = crate::parse::parse(&src, &cp);
        if parsed.file.as_ref().is_some_and(|cf| !cf.applies.is_empty()) {
            out.push(e044(
                tr!("`{}` 自身が `{}` を持っています", "`{}` itself has an `{}`", a.path, crate::kw::APPLY),
                tr!("準用は一段までです。準用の準用は、元の規則の中身をこの規則に書き写してください。", "An apply goes one level. A provision applied through another is written expanded."),
            ));
            done.push(a);
            continue;
        }
        // The callee has to pass the whole check — completeness and the rest, not the type
        // check alone — before its definitions are trusted here.
        let verdict = crate::report(&src, &cp).diags;
        let (cf, cc) = match crate::prepare(&src, &cp) {
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
        if cf.elements.is_some() || cf.fold.is_some() || cf.items.iter().any(|it| matches!(it, Item::Count(_))) {
            out.push(e044(
                tr!("`{}` は並びを順に見ていく規則です", "`{}` walks a sequence", a.path),
                tr!("並びを順に見ていく規則の準用は、まだできません。", "Applying a rule that walks a sequence is not accepted yet."),
            ));
            done.push(a);
            continue;
        }
        let cal_alias = cf.name.ascii.clone().unwrap_or_else(|| an.clone());

        // The bindings: every callee input exactly once, nothing the callee lacks.
        let mut bad = false;
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
                    .note(tr!("元の規則の入力は全部、`<元の入力> = <この規則の値>` の行で読み替えを書きます。", "Every callee input is bound explicitly by a `<input> = <value>` line; that is the substitution.")),
                );
                bad = true;
            }
        }
        for b in &a.bindings {
            if !cf.inputs.iter().any(|i| i.name.text == b.input) {
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
        // What the expansion does not carry yet: an enum of the callee's own that reaches an
        // output column or an output. Its values would have no spelling in this rule.
        let callee_enum = |ty: &Ty| -> bool {
            match ty {
                Ty::Enum(e) => cf.enums.iter().any(|d| d.name.text == *e),
                Ty::Opt(inner) => matches!(inner.as_ref(), Ty::Enum(e) if cf.enums.iter().any(|d| d.name.text == *e)),
                _ => false,
            }
        };
        for it in &cf.items {
            if let Item::Table(t) = it {
                for oc in &t.outputs {
                    if cc.ty_of(&oc.name.text).is_some_and(|ty| callee_enum(&ty)) {
                        out.push(e044(
                            tr!("表 {} の出力 `{}` は、元の規則が宣言した列挙です", "the output `{}` of table {} is an enum the callee declares", t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(), oc.name.text),
                            tr!("元の規則の列挙を出力に持つ表や節は、まだ準用できません。入力の列挙なら `with` で値を対応づけられます。", "Expanding a table or clause that produces one of the callee's enums is not accepted yet. An enum input is mapped with `with`."),
                        ));
                        bad = true;
                    }
                }
            }
        }
        if bad {
            done.push(a);
            continue;
        }

        // The renamer: subs, maps, groups.
        let mut subs: HashMap<String, Sub> = HashMap::new();
        let mut maps: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
        let mut input_enum: HashMap<String, String> = HashMap::new();
        for cin in &cf.inputs {
            let b = a.bindings.iter().find(|b| b.input == cin.name.text).unwrap();
            subs.insert(cin.name.text.clone(), match &b.value {
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
                        if !mp.contains_key(v) && caller_vals.as_ref().is_none_or(|vs| vs.contains(v)) && b.map.iter().all(|(from, _)| from != v) {
                            mp.insert(v.clone(), vec![v.clone()]);
                        }
                    }
                    maps.insert(cin.name.text.clone(), mp);
                }
            }
        }
        let rn = Renamer {
            apply: &an,
            alias: cal_alias,
            at: a.span.clone(),
            subs,
            maps,
            groups: cc.groups.iter().map(|(g, (_, ms))| (g.clone(), ms.clone())).collect(),
            input_enum,
        };

        // The inlined items.
        let mut items: Vec<Item> = Vec::new();
        for it in &cf.items {
            match it {
                Item::Derived(d) => items.push(Item::Derived(DerivedDecl {
                    name: rn.name(&d.name),
                    ty: d.ty.clone(),
                    expr: rn.expr(&d.expr),
                    range: d.range.clone(),
                    span: rn.sp(),
                    cite: rn.cite(&d.cite),
                })),
                Item::Define(d) => items.push(Item::Define(DefineDecl {
                    name: rn.name(&d.name),
                    ty: d.ty.clone(),
                    expr: rn.expr(&d.expr),
                    span: rn.sp(),
                    cite: rn.cite(&d.cite),
                })),
                Item::Count(_) => {}
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
        // the `apply` line. It goes.
        prune(&mut items, &cf.outputs.iter().map(|o| rn.text(&o.name.text)).collect::<Vec<_>>(), cf.result.as_ref().map(|r| rn.expr(&r.expr)));
        // The callee's outputs: its first one may be a `result` expression, the rest are
        // bindings of their own names; each becomes a definition here, under the name this
        // rule gives it, rounded as the callee declared.
        for (oi, od) in cf.outputs.iter().enumerate() {
            let inner = rn.text(&od.name.text);
            if let (Some(r), 0) = (&cf.result, oi) {
                items.push(Item::Define(DefineDecl {
                    name: Name { text: inner.clone(), ascii: Some(format!("{}_{}", rn.alias, od.name.ascii.clone().unwrap_or_else(|| od.name.text.clone()))), span: rn.sp() },
                    ty: od.ty.clone(),
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
                    Item::Count(d) => d.name.text == target.text,
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
            let value = Expr::Name(inner.clone(), rn.sp());
            let expr = match &od.rounding {
                Some(rd) => Expr::Call(rd.mode.clone(), vec![value, Expr::Lit(Lit::Num(rd.grid.clone()), rn.sp())], rn.sp()),
                None => value,
            };
            // The output's own alias is the name this rule declares it under, if it is an
            // output here; else the callee's alias, prefixed.
            let ascii = f.outputs.iter().find(|o| o.name.text == target.text).and_then(|o| o.name.ascii.clone()).or_else(|| target.ascii.clone()).or_else(|| Some(format!("{}_{}", rn.alias, od.name.ascii.clone().unwrap_or_else(|| od.name.text.clone()))));
            a.defines.push(target.text.clone());
            items.push(Item::Define(DefineDecl {
                name: Name { text: target.text.clone(), ascii, span: target.span.clone() },
                ty: od.ty.clone(),
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
        // Prefectures the callee's cells name need the enum here too.
        for (p, sp) in &cf.imports {
            if !f.imports.iter().any(|(q, _)| q == p) {
                f.imports.push((p.clone(), sp.clone()));
            }
        }
        let n = items.len();
        let at_idx = a.at.min(f.items.len());
        f.items.splice(at_idx..at_idx, items);
        a.count = n;
        reorder_after_overrides(f, at_idx, n, &an);
        a.callee = Some(Callee {
            rule: cf.name.clone(),
            inputs: cf
                .inputs
                .iter()
                .map(|i| CalleeInput { name: i.name.text.clone(), ty: cc.ty_of(&i.name.text).unwrap_or(Ty::Unknown), range: i.range.clone() })
                .collect(),
            constraints: cf.constraints.clone(),
            enums: cf.enums.iter().map(|e| (e.name.text.clone(), e.values.iter().map(|v| v.text.clone()).collect())).collect(),
        });
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
fn reorder_after_overrides(f: &mut RuleFile, at: usize, n: usize, an: &str) {
    let prefix = format!("{an}:");
    let mut k = at + n;
    while k < f.items.len() {
        let Item::Table(t) = &f.items[k] else {
            k += 1;
            continue;
        };
        // The outputs of the inlined tables this item takes precedence over.
        let mut outs: Vec<String> = Vec::new();
        for r in &t.overrides {
            let joined = match &r.row {
                Some(l) => format!("{}:{l}", r.table),
                None => r.table.clone(),
            };
            for it in &f.items[at..k] {
                if let Item::Table(u) = it {
                    let un = u.name.as_ref().map(|x| x.text.clone()).unwrap_or_default();
                    if un.starts_with(&prefix) && (un == r.table || un == joined) {
                        outs.extend(u.outputs.iter().map(|o| o.name.text.clone()));
                    }
                }
            }
        }
        if outs.is_empty() {
            k += 1;
            continue;
        }
        // Everything between the expansion's start and this item that reads one of those
        // outputs, or reads something that does, in order.
        let mut moving: Vec<usize> = Vec::new();
        let mut names: Vec<String> = outs;
        for j in at..k {
            let reads: Vec<String> = match &f.items[j] {
                Item::Derived(d) => expr_names(&d.expr),
                Item::Define(d) => expr_names(&d.expr),
                Item::Count(_) => Vec::new(),
                Item::Table(u) => {
                    let mut v: Vec<String> = u.inputs.iter().map(|(c, _)| c.clone()).collect();
                    for r in &u.rows {
                        for o in &r.outs {
                            if let OutCell::Name(w) = o {
                                v.push(w.clone());
                            }
                        }
                    }
                    v
                }
            };
            let is_member = matches!(&f.items[j], Item::Table(u) if u.outputs.iter().any(|o| names.contains(&o.name.text)));
            if !is_member && reads.iter().any(|r| names.contains(r)) {
                moving.push(j);
                match &f.items[j] {
                    Item::Derived(d) => names.push(d.name.text.clone()),
                    Item::Define(d) => names.push(d.name.text.clone()),
                    Item::Table(u) => names.extend(u.outputs.iter().map(|o| o.name.text.clone())),
                    Item::Count(_) => {}
                }
            }
        }
        if moving.is_empty() {
            k += 1;
            continue;
        }
        let mut taken: Vec<Item> = Vec::new();
        for &j in moving.iter().rev() {
            taken.push(f.items.remove(j));
        }
        taken.reverse();
        let dest = k - moving.len() + 1;
        let m = taken.len();
        f.items.splice(dest..dest, taken);
        k = dest + m;
    }
}

fn expr_names(e: &Expr) -> Vec<String> {
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
                Item::Count(_) => {}
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
    if base == "都道府県" {
        return crate::prelude::lookup("std/都道府県").map(|(_, vs)| vs);
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
                            Cell::Lit(Lit::Word(_)) | Cell::Set(_) => {
                                let ls: Vec<Lit> = match &cell {
                                    Cell::Lit(l) => vec![l.clone()],
                                    Cell::Set(ls) => ls.clone(),
                                    _ => unreachable!(),
                                };
                                let words = rn.map_words(name, &ls);
                                match words.len() {
                                    0 => continue 'rows,
                                    1 => Cell::Lit(Lit::Word(words[0].clone())),
                                    _ => Cell::Set(words.into_iter().map(Lit::Word).collect()),
                                }
                            }
                            Cell::Not(ls) => {
                                let words = rn.map_words(name, ls);
                                if words.is_empty() {
                                    Cell::DontCare
                                } else {
                                    Cell::Not(words.into_iter().map(Lit::Word).collect())
                                }
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
                ty: o.ty.clone().or_else(|| callee_outputs.iter().find(|od| od.name.text == o.name.text).map(|od| od.ty.clone())),
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
    })
}

fn cell_names(c: &Cell) -> Vec<String> {
    let ls: Vec<&Lit> = match c {
        Cell::Lit(l) => vec![l],
        Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
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
            // Types.
            match &b.value {
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
                                    if !caller_vals.contains(from) {
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
            // Range: what this rule can pass has to stay inside what the callee proved over.
            let Some(r) = &cin.range else { continue };
            let Some((clo, chi)) = bounds_of(r, &cty) else { continue };
            let reach = interval(b);
            let (lo, hi) = match reach {
                Some((lo, hi)) => (lo, hi),
                None => {
                    out.push(
                        Diag::error("E043", tr!("`{}` に渡す値の取りうる範囲が分かりません", "The interval of what is passed to `{}` is not known", cin.name))
                            .at(at(b.span.line))
                            .mark(b.span.clone(), "")
                            .note(tr!("元の規則の `{}` は range {} の上で検査されています。渡す側にも範囲が要ります。", "The callee's `{}` was checked over range {}; the value passed needs a range.", cin.name, range_text(r))),
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
                        .note(tr!("例: {} = {w} は、`{}` の `{}` の range {} の外です。", "For example {} = {w} is outside range {} of `{}` in `{}`.", cin.name, a.path, cin.name, range_text(r)))
                        .note(tr!(
                            "元の規則の完全性はその範囲の上で証明されていて、外の値には定義がありません。渡す側の範囲を狭めるか、はみ出す部分をこの規則の節で定めてください。どちらにするかは業務の判断です。",
                            "The callee's completeness was proved over that range; outside it there is no definition. Narrow the range on this side, or define that region in a clause of this rule. Which is a business decision."
                        )),
                );
            }
        }
        // Constraints of the callee: proved from this rule's constraints or intervals.
        for k in &cal.constraints {
            let bind = |n: &str| a.bindings.iter().find(|b| b.input == n);
            let (Some(bl), Some(br)) = (bind(&k.left), bind(&k.right)) else { continue };
            let names = match (&bl.value, &br.value) {
                (BindValue::Name(x), BindValue::Name(y)) => Some((x.clone(), y.clone())),
                _ => None,
            };
            let mirrored = |op: CmpOp| match op {
                CmpOp::Le => CmpOp::Ge,
                CmpOp::Ge => CmpOp::Le,
                CmpOp::Lt => CmpOp::Gt,
                CmpOp::Gt => CmpOp::Lt,
            };
            let declared = names.as_ref().is_some_and(|(x, y)| {
                x == y && matches!(k.op, CmpOp::Le | CmpOp::Ge)
                    || f.constraints.iter().any(|kc| (kc.left == *x && kc.op == k.op && kc.right == *y) || (kc.left == *y && kc.op == mirrored(k.op) && kc.right == *x))
            });
            // Or the intervals decide it: the whole of one side lies on the right side of the
            // whole of the other.
            let by_interval = match (interval(bl), interval(br)) {
                (Some((xl, xh)), Some((yl, yh))) => match k.op {
                    CmpOp::Le => xh.zip(yl).is_some_and(|(p, q)| p.cmp_to(q) != std::cmp::Ordering::Greater),
                    CmpOp::Lt => xh.zip(yl).is_some_and(|(p, q)| p.cmp_to(q) == std::cmp::Ordering::Less),
                    CmpOp::Ge => xl.zip(yh).is_some_and(|(p, q)| p.cmp_to(q) != std::cmp::Ordering::Less),
                    CmpOp::Gt => xl.zip(yh).is_some_and(|(p, q)| p.cmp_to(q) == std::cmp::Ordering::Greater),
                },
                _ => false,
            };
            if !declared && !by_interval {
                let shown = |b: &Binding| match &b.value {
                    BindValue::Name(n) => n.clone(),
                    BindValue::Lit(l) => lit_text(l),
                };
                out.push(
                    Diag::error("E043", tr!("元の規則の制約 `{} {} {}` が、この規則の宣言から導けません", "The callee's constraint `{} {} {}` does not follow from this rule's declarations", k.left, k.op.word(), k.right))
                        .at(at(a.span.line))
                        .mark(a.span.clone(), "")
                        .note(tr!("元の規則は `{}` と `{}` にその関係があると宣言していて、完全性はそれを前提に検査されています。ここでは `{}` と `{}` を渡します。", "The callee declares that relation between `{}` and `{}`, and its completeness was checked believing it. Here `{}` and `{}` are passed.", k.left, k.right, shown(bl), shown(br)))
                        .note(tr!("同じ関係を `{} {} {} {}` として宣言するか、範囲でそれが成り立つようにしてください。", "Declare the same relation as `{} {} {} {}`, or make the ranges imply it.", crate::kw::CONSTRAINT, shown(bl), k.op.word(), shown(br))),
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
