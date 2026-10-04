//! Names and types (DESIGN 1.2, 1.3; stage 2 of 3.1): E007–E014, E304. Turns a dates file
//! into a [`Model`] the interpreter runs: every name looked up once, every number either
//! written out or an integer input, the dates in an order that computes each after the one
//! it starts from.

use crate::ast::*;
use crate::calendar::Calendar;
use crate::date::{Day, Missing};
use crate::diag::Diag;
use ritsu_base::text::Text;
use crate::kw;
use crate::naming;
use crate::reserved;
use crate::sources::Law;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct In {
    pub name: String,
    pub alias: String,
    pub ty: Ty,
    /// The range, both ends included; days as day numbers.
    pub lo: i64,
    pub hi: i64,
    pub span: Span,
}

impl In {
    /// How a value of this input is written: `2026-04-01` or `3`.
    pub fn show(&self, v: i64) -> String {
        match self.ty {
            Ty::Date => Day(v as i32).to_string(),
            Ty::Int => v.to_string(),
        }
    }

    pub fn size(&self) -> u64 {
        (self.hi - self.lo + 1).max(0) as u64
    }
}

/// A number in an operation, resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum A {
    Lit(i64),
    Input(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ROp {
    /// `± n days`: the sign as ±1.
    Days(i64, A),
    /// `± n business days`; true going forward.
    Business(bool, A),
    /// `± n months` (`per` 1) or `± n years` (`per` 12).
    Months { sign: i64, n: A, per: i64, missing: Option<Missing> },
    DayOfMonth { n: A, sign: i64, k: A, missing: Option<Missing> },
    StartOfMonth(i64, A),
    EndOfMonth(i64, A),
    CloseDay(A, Option<Missing>),
    CloseEndOfMonth,
    Roll(Conv),
    IfClosed(Box<ROp>),
}

impl ROp {
    /// Whether the operation asks the calendar (DESIGN 2.2).
    pub fn needs_calendar(&self) -> bool {
        matches!(self, ROp::Business(..) | ROp::Roll(_) | ROp::IfClosed(_))
    }
}

#[derive(Clone, Debug)]
pub struct O {
    pub op: ROp,
    pub span: Span,
    pub text: String,
    pub stray_else: Option<Span>,
    /// Where `else` is, when it is written.
    pub else_span: Option<Span>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Input,
    Date(usize),
}

#[derive(Clone, Debug)]
pub struct D {
    pub name: String,
    pub alias: String,
    pub span: Span,
    pub start: Start,
    pub start_name: String,
    pub ops: Vec<O>,
    pub at: Option<(At, Span)>,
    /// The inputs the date uses, in the order they are declared.
    pub params: Vec<usize>,
    /// The dates computed to get this one, in order, ending with itself.
    pub chain: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ref {
    Input,
    Date(usize),
}

/// One side of a claim.
#[derive(Clone, Debug)]
pub struct R {
    pub what: Ref,
    pub name: String,
    /// `± n [business] days`: forward, the number, business days.
    pub offset: Option<(bool, A, bool)>,
}

#[derive(Clone, Debug)]
pub enum CK {
    IsOpen(R),
    Monotonic(usize),
    Compare(R, Cmp, R),
}

#[derive(Clone, Debug)]
pub struct C {
    pub name: String,
    pub span: Span,
    pub text: String,
    pub kind: CK,
}

#[derive(Clone, Debug)]
pub struct Ex {
    pub span: Span,
    /// Every input, by index, with its value in the row.
    pub inputs: Vec<(usize, i64)>,
    /// Every date, by index, with the value the row says and where it is.
    pub outputs: Vec<(usize, Day, Span)>,
}

/// A dates file, resolved.
pub struct Model {
    pub file: File,
    pub path: PathBuf,
    pub sha256: String,
    pub cal: Option<Calendar>,
    pub laws: Vec<Law>,
    pub inputs: Vec<In>,
    pub date_input: usize,
    pub dates: Vec<D>,
    /// Every date, each after the one it starts from.
    pub order: Vec<usize>,
    pub claims: Vec<C>,
    pub examples: Vec<Ex>,
}

impl Model {
    pub fn int_inputs(&self) -> Vec<usize> {
        (0..self.inputs.len()).filter(|i| *i != self.date_input).collect()
    }

    pub fn date_in(&self) -> &In {
        &self.inputs[self.date_input]
    }

    /// The number of input combinations: the days of the date's range times the size of
    /// every integer input's.
    pub fn combinations(&self) -> u128 {
        self.inputs.iter().map(|i| i.size() as u128).product()
    }

    pub fn needs_calendar(&self) -> bool {
        let ops = self.dates.iter().flat_map(|d| d.ops.iter()).any(|o| o.op.needs_calendar());
        let claims = self.claims.iter().any(|c| match &c.kind {
            CK::IsOpen(_) => true,
            CK::Compare(a, _, b) => [a, b].iter().any(|r| matches!(r.offset, Some((_, _, true)))),
            CK::Monotonic(_) => false,
        });
        ops || claims
    }
}

struct Ctx<'a> {
    f: &'a File,
    diags: Vec<Diag>,
}

impl Ctx<'_> {
    fn err(&mut self, code: &'static str, s: Span, msg: Text) -> &mut Diag {
        self.diags.push(Diag::error(code, &self.f.path, s.line, s.col, msg).source(&self.f.src));
        self.diags.last_mut().unwrap()
    }
}

/// E009 and E010 for the heading's name, which names the module, schema and package.
pub fn check_header(f: &File) -> Vec<Diag> {
    let mut c = Ctx { f, diags: vec![] };
    alias_checks(&mut c, &f.name, &tr!("ファイル", "the file"), true);
    c.diags
}

/// E010 (an alias missing or of the wrong form) and E009 (an alias or name the language or
/// a target will not take).
fn alias_checks(c: &mut Ctx, n: &Name, what: &Text, module: bool) {
    if kw::is_reserved(&n.text) {
        c.err("E009", n.span, tr!("{}の名前「{}」は koyomi の予約語です", "The name {} of {} is a word of the language", what.ja, n.text; n.text, what.en))
            .notes
            .push(tr!("別の名前を付けてください。", "Choose another name."));
    }
    match &n.alias {
        Some((a, s)) if !is_alias(a) => {
            c.err("E010", *s, tr!("別名 {a} の形が違います。別名は `[a-z][a-z0-9_]*` です", "The alias {a} is not of the form `[a-z][a-z0-9_]*`"))
                .notes
                .push(tr!(
                    "別名は、生成したコードの識別子になります。koyomi は、五つの出力先のどれでも使える形に限っています。",
                    "An alias becomes an identifier in the generated code, so it takes a form all five targets accept."
                ));
            return;
        }
        None if !is_alias(&n.text) => {
            c.err("E010", n.span, tr!("公開する名前「{}」に ASCII の別名がありません", "The public name {} has no ASCII alias", n.text))
                .notes
                .push(tr!(
                    "生成したコードの識別子になる名前には、`{}(alias)` のように丸括弧で別名を付けてください。名前がもとから `[a-z][a-z0-9_]*` の形なら要りません。",
                    "A name that becomes an identifier in the generated code takes an alias in parentheses, like `{}(alias)`; a name already of the form `[a-z][a-z0-9_]*` needs none.",
                    n.text
                ));
            return;
        }
        _ => {}
    }
    let Some(a) = n.ascii() else { return };
    let span = n.alias.as_ref().map(|(_, s)| *s).unwrap_or(n.span);
    if n.alias.is_none() && kw::is_reserved(a) {
        // The name is its own alias, and E009 has been said of the name.
        return;
    }
    if kw::is_reserved(a) {
        c.err("E009", span, tr!("別名 {a} は koyomi の予約語です", "The alias {a} is a word of the language"));
        return;
    }
    let refusing = reserved::refusing(a);
    if !refusing.is_empty() {
        let list = Text::list(&refusing.iter().map(|t| Text::same(t.to_string())).collect::<Vec<_>>());
        c.err("E009", span, tr!("別名 {a} は {} の予約語で、生成したコードで使えません", "The alias {a} is a reserved word of {}, so the generated code cannot use it", list.ja; list.en))
            .notes
            .push(tr!("別の別名を付けてください。", "Choose another alias."));
        return;
    }
    if module && naming::MODULES.contains(&a) {
        c.err("E009", span, tr!("別名 {a} は、生成したコードが読み込むモジュールやスキーマの名前とぶつかります", "The alias {a} collides with a module or schema the generated code reads"))
            .notes
            .push(tr!(
                "ファイルの別名は、Python と Rust のモジュール、PostgreSQL のスキーマの名前になります。Python の `datetime`・`json`・`sys`・`typing`、Rust の `std`・`core`・`alloc`、PostgreSQL の `pg_catalog` などとは別の名前にしてください。",
                "A file's alias names a Python and a Rust module and a PostgreSQL schema; keep it apart from Python's `datetime`, `json`, `sys` and `typing`, Rust's `std`, `core` and `alloc`, and PostgreSQL's `pg_catalog` and its kin."
            ));
    }
    if !module && naming::GENERATED.contains(&a) {
        c.err("E009", span, tr!("別名 {a} は、生成したコードが使う名前とぶつかります", "The alias {a} collides with a name the generated code defines"))
            .notes
            .push(tr!(
                "生成したコードは `is_open`、`Date`、`KoyomiError`、`ParseDate` を置き、Go は `err` を使い、Python は `date` を読み込んで `range`、`str`、`int`、`bool`、`tuple`、`isinstance`、`frozenset`、`super` を呼びます。",
                "The generated code defines `is_open`, `Date`, `KoyomiError` and `ParseDate`; the Go uses `err`; the Python imports `date` and calls `range`, `str`, `int`, `bool`, `tuple`, `isinstance`, `frozenset` and `super`."
            ));
    }
}

/// Turn a parsed dates file into a model (stage 2 of DESIGN 3.1). The calendar and the laws
/// are filled in by the check.
pub fn resolve(f: File, path: PathBuf, sha256: String) -> Result<Model, Vec<Diag>> {
    let mut c = Ctx { f: &f, diags: vec![] };
    alias_checks(&mut c, &f.name, &tr!("ファイル", "the file"), true);
    // Sources: names once each.
    let mut src_seen: HashMap<&str, Span> = HashMap::new();
    for s in &f.sources {
        if let Some(prev) = src_seen.get(s.name.as_str()) {
            let l = prev.line;
            c.err("E007", s.span, tr!("出典「{}」が二度宣言されています（{l} 行目にもあります）", "The source {} is declared twice (line {l} too)", s.name; s.name));
        } else {
            src_seen.insert(&s.name, s.span);
        }
    }
    // Inputs and dates share one space of names, and their aliases another.
    let mut names: HashMap<String, Span> = HashMap::new();
    let mut aliases: HashMap<String, Span> = HashMap::new();
    let mut declare = |c: &mut Ctx, n: &Name| {
        if let Some(prev) = names.get(&n.text) {
            let l = prev.line;
            c.err("E007", n.span, tr!("名前「{}」が二度宣言されています（{l} 行目にもあります）", "The name {} is declared twice (line {l} too)", n.text; n.text));
            return;
        }
        names.insert(n.text.clone(), n.span);
        if let Some(a) = n.ascii()
            && is_alias(a)
        {
            let span = n.alias.as_ref().map(|(_, s)| *s).unwrap_or(n.span);
            if let Some(prev) = aliases.get(a) {
                let l = prev.line;
                c.err("E007", span, tr!("別名 {a} が二度使われています（{l} 行目にもあります）", "The alias {a} is used twice (line {l} too)"));
            } else {
                aliases.insert(a.to_string(), span);
            }
        }
    };
    for i in &f.inputs {
        declare(&mut c, &i.name);
        alias_checks(&mut c, &i.name, &tr!("入力", "the input"), false);
    }
    for d in &f.dates {
        declare(&mut c, &d.name);
        alias_checks(&mut c, &d.name, &tr!("日付", "the date"), false);
    }
    // `<alias>_at` is the function of a date's time.
    for d in &f.dates {
        if d.at.is_some()
            && let Some(a) = d.name.ascii()
        {
            let at = naming::at_alias(a);
            if let Some(s) = aliases.get(&at) {
                let n = d.name.text.clone();
                c.err("E009", *s, tr!(
                    "別名 {at} は、日付「{n}」の時刻の関数 {at} とぶつかります",
                    "The alias {at} collides with {at}, the function of the time of {n}"
                ));
            }
        }
    }
    // The inputs' types and ranges.
    let date_inputs: Vec<&Input> = f.inputs.iter().filter(|i| i.ty == Ty::Date).collect();
    if date_inputs.len() != 1 && !f.inputs.is_empty() {
        let s = date_inputs.get(1).map(|i| i.span).unwrap_or(f.inputs[0].span);
        let n = date_inputs.len();
        c.err("E012", s, tr!("`date` の入力が {n} 個あります。ちょうど一つにしてください", "There are {n} `date` inputs; there must be exactly one"))
            .notes
            .push(tr!(
                "総当たりで確かめられる大きさに抑えるためです。日付が二つあると、100 年どうしで 13 億通りを超えます。二つめの日付が要る計算は、日数を整数の入力として受け取れば書けます（DESIGN 1.3）。",
                "This keeps the check exhaustive: two dates of 100 years each are over 1.3 billion combinations. A computation that needs a second date can take the number of days between as an integer input (DESIGN 1.3)."
            ));
    }
    let mut inputs: Vec<In> = Vec::new();
    for i in &f.inputs {
        let alias = i.name.ascii().unwrap_or("").to_string();
        let mut lo_v = 0i64;
        let mut hi_v = -1i64;
        let mut ok = true;
        for (end, which) in [(&i.lo, 0), (&i.hi, 1)] {
            match end {
                None => {
                    ok = false;
                    let (ja, en) = if which == 0 { ("下の端（`>=`）", "lower end (`>=`)") } else { ("上の端（`<=`）", "upper end (`<=`)") };
                    let example = if i.ty == Ty::Date { "range >=2026-01-01 <=2026-12-31" } else { "range >=1 <=31" };
                    c.err("E013", i.range_span, tr!(
                        "入力「{}」の範囲に{ja}がありません",
                        "The range of the input {} has no {en}",
                        i.name.text;
                        i.name.text
                    ))
                    .notes
                    .push(tr!(
                        "どの入力にも、範囲の両端を書いてください（`{example}` のように）。範囲は、総当たりで計算する範囲で、生成したコードの入口のガードにもなります。",
                        "Every input has both ends of its range (like `{example}`): the range is what the check walks, and the guard at the entrance of the generated code."
                    ));
                }
                Some((lit, s)) => {
                    let v = match (i.ty, lit) {
                        (Ty::Date, Lit::Date(d)) => d.0 as i64,
                        (Ty::Int, Lit::Int(n)) => *n,
                        (Ty::Date, Lit::Int(_)) => {
                            ok = false;
                            c.err("E011", *s, tr!("入力「{}」は日付なので、範囲の端も日付で書いてください", "The input {} is a date, so the ends of its range are dates", i.name.text; i.name.text));
                            continue;
                        }
                        (Ty::Int, Lit::Date(_)) => {
                            ok = false;
                            c.err("E011", *s, tr!("入力「{}」は整数なので、範囲の端も数で書いてください", "The input {} is an integer, so the ends of its range are numbers", i.name.text; i.name.text));
                            continue;
                        }
                    };
                    if which == 0 { lo_v = v } else { hi_v = v }
                }
            }
        }
        if ok && lo_v > hi_v {
            let (a, b) = (i.lo.as_ref().unwrap(), i.hi.as_ref().unwrap());
            let show = |l: &Lit| match l {
                Lit::Date(d) => d.to_string(),
                Lit::Int(n) => n.to_string(),
            };
            c.err("E013", i.range_span, tr!(
                "入力「{}」の範囲 {}〜{} は空です",
                "The range {}..{} of the input {} is empty",
                i.name.text,
                show(&a.0),
                show(&b.0);
                show(&a.0),
                show(&b.0),
                i.name.text
            ));
        }
        inputs.push(In { name: i.name.text.clone(), alias, ty: i.ty, lo: lo_v, hi: hi_v, span: i.span });
    }
    let date_input = f.inputs.iter().position(|i| i.ty == Ty::Date).unwrap_or(0);
    // A name declared twice means its first declaration (E007 has said so of the second).
    let mut input_ix: HashMap<&str, usize> = HashMap::new();
    for (k, i) in f.inputs.iter().enumerate() {
        input_ix.entry(i.name.text.as_str()).or_insert(k);
    }
    let mut date_ix: HashMap<&str, usize> = HashMap::new();
    for (k, d) in f.dates.iter().enumerate() {
        date_ix.entry(d.name.text.as_str()).or_insert(k);
    }
    let twice = c.diags.iter().any(|d| d.code == "E007");

    let names = Names { f: &f, inputs: &inputs, input_ix: &input_ix, date_ix: &date_ix };

    let mut dates: Vec<D> = Vec::new();
    for d in &f.dates {
        let (sname, sspan) = &d.start;
        let start = match (input_ix.get(sname.as_str()), date_ix.get(sname.as_str())) {
            (Some(k), _) if f.inputs[*k].ty == Ty::Date => Start::Input,
            (Some(_), _) => {
                c.err("E011", *sspan, tr!("入力「{sname}」は整数です。日付は、日付の入力かほかの日付から始めてください", "The input {sname} is an integer; a date starts from the date input or another date"));
                Start::Input
            }
            (None, Some(k)) => Start::Date(*k),
            (None, None) => {
                c.err("E008", *sspan, tr!("「{sname}」は宣言されていません", "The name {sname} is not declared")).notes.push(tr!(
                    "日付は、`inputs` の日付か、`date` で宣言したほかの日付から始めてください。",
                    "A date starts from the date under `inputs` or from another date declared with `date`."
                ));
                Start::Input
            }
        };
        let mut ops = Vec::new();
        for o in &d.ops {
            let op = names.conv(&mut c, &o.op, o.span);
            let else_span = match &o.op {
                Op::Months(_, _, m) | Op::Years(_, _, m) | Op::DayOfMonth(_, _, _, m) | Op::CloseDay(_, m) => m.map(|x| x.1),
                Op::IfClosed(inner) => match inner.as_ref() {
                    Op::Months(_, _, m) | Op::Years(_, _, m) | Op::DayOfMonth(_, _, _, m) | Op::CloseDay(_, m) => m.map(|x| x.1),
                    _ => None,
                },
                _ => None,
            };
            ops.push(O { op, span: o.span, text: o.text.clone(), stray_else: o.stray_else.map(|x| x.1), else_span });
        }
        dates.push(D {
            name: d.name.text.clone(),
            alias: d.name.ascii().unwrap_or("").to_string(),
            span: d.span,
            start,
            start_name: sname.clone(),
            ops,
            at: d.at.as_ref().map(|(a, s, _)| (*a, *s)),
            params: vec![],
            chain: vec![],
        });
    }
    // E014: dates that start from each other in a circle.
    let mut order: Vec<usize> = Vec::new();
    let mut state = vec![0u8; dates.len()]; // 0 new, 1 on the way, 2 done
    fn visit(k: usize, dates: &[D], state: &mut [u8], order: &mut Vec<usize>, path: &mut Vec<usize>) -> Option<Vec<usize>> {
        if state[k] == 2 {
            return None;
        }
        if state[k] == 1 {
            let at = path.iter().position(|x| *x == k).unwrap_or(0);
            return Some(path[at..].to_vec());
        }
        state[k] = 1;
        path.push(k);
        if let Start::Date(s) = dates[k].start
            && let Some(cy) = visit(s, dates, state, order, path)
        {
            return Some(cy);
        }
        path.pop();
        state[k] = 2;
        order.push(k);
        None
    }
    let mut cycle_found = false;
    for k in 0..dates.len() {
        let mut path = Vec::new();
        if let Some(cy) = visit(k, &dates, &mut state, &mut order, &mut path) {
            if !cycle_found {
                let names: Vec<String> = cy.iter().map(|i| dates[*i].name.clone()).chain(std::iter::once(dates[cy[0]].name.clone())).collect();
                let s = dates[cy[0]].span;
                c.err("E014", s, tr!("日付が循環しています: {}", "The dates go round in a circle: {}", names.join(" ← "); names.join(" <- ")))
                    .notes
                    .push(tr!("どの日付も、最後は `inputs` の日付から始まるようにしてください。", "Every date has to start, in the end, from the date under `inputs`."));
            }
            cycle_found = true;
            break;
        }
    }
    if !cycle_found {
        for k in &order.clone() {
            let mut chain = match dates[*k].start {
                Start::Date(s) => dates[s].chain.clone(),
                Start::Input => vec![],
            };
            chain.push(*k);
            let mut params = vec![date_input];
            for x in &chain {
                for o in &dates[*x].ops {
                    collect_inputs(&o.op, &mut params);
                }
            }
            params.sort();
            params.dedup();
            dates[*k].chain = chain;
            dates[*k].params = params;
        }
    }

    // Claims.
    let mut claim_seen: HashMap<&str, Span> = HashMap::new();
    let mut claims = Vec::new();
    let date_ref = |c: &mut Ctx, r: &DateRef| -> Option<R> {
        let what = match (input_ix.get(r.name.as_str()), date_ix.get(r.name.as_str())) {
            (Some(k), _) if f.inputs[*k].ty == Ty::Date => Ref::Input,
            (Some(_), _) => {
                let n = &r.name;
                c.err("E011", r.span, tr!("「{n}」は整数です。条件で比べるのは日付です", "{n} is an integer; a claim compares dates"));
                return None;
            }
            (None, Some(k)) => Ref::Date(*k),
            (None, None) => {
                let n = &r.name;
                c.err("E008", r.span, tr!("「{n}」は宣言されていません", "The name {n} is not declared"));
                return None;
            }
        };
        let offset = r.offset.as_ref().map(|(s, n, b)| (*s == Sign::Plus, names.num(c, n, Use::Count, r.span), *b));
        Some(R { what, name: r.name.clone(), offset })
    };
    for cl in &f.claims {
        if let Some(prev) = claim_seen.get(cl.name.as_str()) {
            let l = prev.line;
            c.err("E007", cl.span, tr!("条件の名前 {} が二度使われています（{l} 行目にもあります）", "The claim name {} is used twice (line {l} too)", cl.name; cl.name));
        } else {
            claim_seen.insert(&cl.name, cl.span);
        }
        let kind = match &cl.kind {
            ClaimKind::IsOpen(r) => date_ref(&mut c, r).map(CK::IsOpen),
            ClaimKind::Monotonic(n, s) => match date_ix.get(n.as_str()) {
                Some(k) => Some(CK::Monotonic(*k)),
                None if input_ix.contains_key(n.as_str()) => {
                    c.err("E011", *s, tr!("「{n}」は入力です。`is monotonic` は日付について書いてください", "{n} is an input; `is monotonic` is said of a date"));
                    None
                }
                None => {
                    c.err("E008", *s, tr!("「{n}」は宣言されていません", "The name {n} is not declared"));
                    None
                }
            },
            ClaimKind::Compare(a, cmp, b) => match (date_ref(&mut c, a), date_ref(&mut c, b)) {
                (Some(x), Some(y)) => Some(CK::Compare(x, *cmp, y)),
                _ => None,
            },
        };
        if let Some(kind) = kind {
            claims.push(C { name: cl.name.clone(), span: cl.span, text: cl.text.clone(), kind });
        }
    }

    // Examples: a column for every input and every date (E304).
    let mut examples = Vec::new();
    if let Some(ex) = &f.examples
        && !ex.columns.is_empty()
    {
        let mut cols: Vec<Option<(bool, usize)>> = Vec::new();
        let mut seen_cols: HashMap<&str, Span> = HashMap::new();
        for col in &ex.columns {
            if let Some(prev) = seen_cols.get(col.name.as_str()) {
                let l = prev.col;
                c.err("E007", col.span, tr!("列「{}」が二度あります（{l} 文字目にもあります）", "The column {} is there twice (at column {l} too)", col.name; col.name));
                cols.push(None);
                continue;
            }
            seen_cols.insert(&col.name, col.span);
            let n = &col.name;
            match (col.output, input_ix.get(n.as_str()), date_ix.get(n.as_str())) {
                (false, Some(k), _) => cols.push(Some((false, *k))),
                (true, _, Some(k)) => cols.push(Some((true, *k))),
                (true, Some(_), _) => {
                    c.err("E011", col.span, tr!("「{n}」は入力です。`->` を付けるのは日付の列です", "{n} is an input; `->` marks the column of a date"));
                    cols.push(None);
                }
                (false, _, Some(_)) => {
                    c.err("E011", col.span, tr!("「{n}」は日付です。日付の列には `-> {n}` と書いてください", "{n} is a date; its column is written `-> {n}`"));
                    cols.push(None);
                }
                _ => {
                    c.err("E008", col.span, tr!("「{n}」は宣言されていません", "The name {n} is not declared"));
                    cols.push(None);
                }
            }
        }
        let missing_in: Vec<&str> = f.inputs.iter().enumerate().filter(|(k, _)| !cols.contains(&Some((false, *k)))).map(|(_, i)| i.name.text.as_str()).collect();
        let missing_out: Vec<&str> = f.dates.iter().enumerate().filter(|(k, _)| !cols.contains(&Some((true, *k)))).map(|(_, d)| d.name.text.as_str()).collect();
        if (!missing_in.is_empty() || !missing_out.is_empty()) && !twice {
            let mut all: Vec<String> = missing_in.iter().map(|s| format!("`{s}`")).collect();
            all.extend(missing_out.iter().map(|s| format!("`-> {s}`")));
            let list = all.join(", ");
            let list_ja = all.join("、");
            c.err("E304", ex.span, tr!("例に {list_ja} の列がありません", "The examples have no column for {list}"))
                .notes
                .push(tr!(
                    "例には、入力と日付の列を全部書いてください。参照インタプリタと生成したコードが同じ誤りを持てば突き合わせは通るので、その誤りを見つけられるのは人が書いた値だけです（DESIGN 1.11）。",
                    "Examples have a column for every input and every date: if the reference interpreter and the generated code shared a mistake, comparing them would pass, and only values a person wrote can catch it (DESIGN 1.11)."
                ));
        }
        for row in &ex.rows {
            let mut e = Ex { span: row.span, inputs: vec![], outputs: vec![] };
            for (k, (lit, s)) in row.cells.iter().enumerate() {
                let Some(Some((out, ix))) = cols.get(k) else { continue };
                let want_date = *out || f.inputs[*ix].ty == Ty::Date;
                match (lit, want_date) {
                    (Lit::Date(d), true) => {
                        if *out {
                            e.outputs.push((*ix, *d, *s));
                        } else {
                            e.inputs.push((*ix, d.0 as i64));
                        }
                    }
                    (Lit::Int(n), false) => e.inputs.push((*ix, *n)),
                    (_, true) => {
                        c.err("E011", *s, tr!("この列は日付です", "This column holds dates"));
                    }
                    (_, false) => {
                        c.err("E011", *s, tr!("この列は整数です", "This column holds integers"));
                    }
                }
            }
            examples.push(e);
        }
    }

    let diags = std::mem::take(&mut c.diags);
    if diags.iter().any(|d| d.is_error()) {
        return Err(diags);
    }
    Ok(Model { file: f.clone(), path, sha256, cal: None, laws: vec![], inputs, date_input, dates, order, claims, examples })
}

/// How a number is used: added (0 or more), or as a day of the month (1..31).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    Count,
    DayOfMonth,
}

/// What the numbers of the operations are looked up in.
struct Names<'a> {
    f: &'a File,
    inputs: &'a [In],
    input_ix: &'a HashMap<&'a str, usize>,
    date_ix: &'a HashMap<&'a str, usize>,
}

impl Names<'_> {
    /// A number: written out, or an integer input whose range fits its use. `at` is where the
    /// operation is, for a number written out.
    fn num(&self, c: &mut Ctx, a: &Arg, what: Use, at: Span) -> A {
        match a {
            Arg::Lit(n) => {
                if what == Use::DayOfMonth && !(1..=31).contains(n) {
                    c.err("E013", at, tr!("日 {n} は 1〜31 の外です", "The day {n} is outside 1..31"));
                }
                A::Lit(*n)
            }
            Arg::Name(n, s) => match self.input_ix.get(n.as_str()) {
                Some(k) if self.f.inputs[*k].ty == Ty::Int => {
                    let inp = &self.inputs[*k];
                    match what {
                        Use::Count if inp.lo < 0 => {
                            let lo = inp.lo;
                            c.err("E013", *s, tr!(
                                "「{n}」を足す数に使っていますが、範囲が負の数 {lo} を含みます",
                                "{n} is used as a number to add, and its range takes the negative number {lo}"
                            ))
                            .notes
                            .push(tr!(
                                "足す数と引く数は 0 以上にし、向きは `+` と `-` で書いてください（DESIGN 1.6）。",
                                "The number added is 0 or more; the direction is written with `+` or `-` (DESIGN 1.6)."
                            ));
                        }
                        Use::DayOfMonth if inp.lo < 1 || inp.hi > 31 => {
                            let (lo, hi) = (inp.lo, inp.hi);
                            c.err("E013", *s, tr!(
                                "「{n}」を日に使っていますが、範囲 {lo}〜{hi} が 1〜31 からはみ出します",
                                "{n} is used as a day of the month, and its range {lo}..{hi} goes outside 1..31"
                            ));
                        }
                        _ => {}
                    }
                    A::Input(*k)
                }
                Some(_) => {
                    c.err("E011", *s, tr!("「{n}」は日付です。ここには数か整数の入力を書いてください", "{n} is a date; a number or an integer input goes here"));
                    A::Lit(0)
                }
                None if self.date_ix.contains_key(n.as_str()) => {
                    c.err("E011", *s, tr!("「{n}」は日付です。ここには数か整数の入力を書いてください", "{n} is a date; a number or an integer input goes here"));
                    A::Lit(0)
                }
                None => {
                    c.err("E008", *s, tr!("「{n}」は宣言されていません", "The name {n} is not declared")).notes.push(tr!(
                        "ここに書ける名前は、`inputs` で宣言した整数（`int`）の入力です。",
                        "A name here is an integer (`int`) input declared under `inputs`."
                    ));
                    A::Lit(0)
                }
            },
        }
    }

    fn conv(&self, c: &mut Ctx, o: &Op, at: Span) -> ROp {
        match o {
            Op::Days(s, n) => ROp::Days(s.apply(1), self.num(c, n, Use::Count, at)),
            Op::BusinessDays(s, n) => ROp::Business(*s == Sign::Plus, self.num(c, n, Use::Count, at)),
            Op::Months(s, n, m) => ROp::Months { sign: s.apply(1), n: self.num(c, n, Use::Count, at), per: 1, missing: m.map(|x| x.0) },
            Op::Years(s, n, m) => ROp::Months { sign: s.apply(1), n: self.num(c, n, Use::Count, at), per: 12, missing: m.map(|x| x.0) },
            Op::DayOfMonth(n, s, k, m) => ROp::DayOfMonth {
                n: self.num(c, n, Use::DayOfMonth, at),
                sign: s.apply(1),
                k: self.num(c, k, Use::Count, at),
                missing: m.map(|x| x.0),
            },
            Op::StartOfMonth(s, k) => ROp::StartOfMonth(s.apply(1), self.num(c, k, Use::Count, at)),
            Op::EndOfMonth(s, k) => ROp::EndOfMonth(s.apply(1), self.num(c, k, Use::Count, at)),
            Op::CloseDay(n, m) => ROp::CloseDay(self.num(c, n, Use::DayOfMonth, at), m.map(|x| x.0)),
            Op::CloseEndOfMonth => ROp::CloseEndOfMonth,
            Op::Roll(cv) => ROp::Roll(*cv),
            Op::IfClosed(inner) => ROp::IfClosed(Box::new(self.conv(c, inner, at))),
        }
    }
}

fn collect_inputs(op: &ROp, out: &mut Vec<usize>) {
    let mut push = |a: &A| {
        if let A::Input(k) = a {
            out.push(*k);
        }
    };
    match op {
        ROp::Days(_, n) | ROp::Business(_, n) | ROp::StartOfMonth(_, n) | ROp::EndOfMonth(_, n) | ROp::CloseDay(n, _) => push(n),
        ROp::Months { n, .. } => push(n),
        ROp::DayOfMonth { n, k, .. } => {
            push(n);
            push(k);
        }
        ROp::CloseEndOfMonth | ROp::Roll(_) => {}
        ROp::IfClosed(inner) => collect_inputs(inner, out),
    }
}

/// The inputs a claim's sides and offsets use, with the dates' own.
pub fn claim_params(m: &Model, c: &C) -> Vec<usize> {
    let mut out = vec![m.date_input];
    let mut side = |r: &R| {
        if let Ref::Date(k) = r.what {
            out.extend(m.dates[k].params.iter().copied());
        }
        if let Some((_, A::Input(k), _)) = r.offset {
            out.push(k);
        }
    };
    match &c.kind {
        CK::IsOpen(r) => side(r),
        CK::Monotonic(k) => out.extend(m.dates[*k].params.iter().copied()),
        CK::Compare(a, _, b) => {
            side(a);
            side(b);
        }
    }
    out.sort();
    out.dedup();
    out
}
