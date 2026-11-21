//! The form (DESIGN 4): every number, amount and date has its range and every text its limit
//! (E101, E102), and each mark fits its field's type (E009); a computed amount says how it rounds
//! (E104); what is decided when the request is made reads only what the requester fills in
//! (E106); a field approvers may edit that the route reads is said (W108), and so is a field that
//! asks for a secret (W109).

use crate::ast::*;
use crate::diag::Diag;
use crate::names::Scope;
use crate::types::{Ty, Typer, agree, lit_unit, written};
use ritsu_base::text::Text;
use ritsu_units::{Dim, Rat};
use std::collections::{BTreeMap, BTreeSet};

pub fn check(s: &Scope, out: &mut Vec<Diag>) {
    for f in s.fields() {
        field(s, f, out);
        for c in &f.children {
            field(s, c, out);
        }
    }
    for m in &s.file.masters {
        for f in &m.fields {
            field(s, f, out);
        }
    }
    rounding(s, out);
    late(s, out);
    edits(s, out);
    secrets(s, out);
}

fn is_num(f: &Field) -> bool {
    matches!(f.ty, TypeExpr::Num(_))
}

fn is_text(f: &Field) -> bool {
    matches!(f.ty, TypeExpr::Text | TypeExpr::LongText)
}

/// The marks of one field, its range and its length.
fn field(s: &Scope, f: &Field, out: &mut Vec<Diag>) {
    let (n, ty) = (&f.name.text, f.ty.word());
    // what each mark is written on
    if let Some(r) = &f.range
        && !(is_num(f) || f.ty == TypeExpr::Date)
    {
        out.push(s.err("E009", r.span, tr!("`range` は数、金額、日付に書きます。`{n}` は `{ty}` です", "`range` is written on a number, an amount or a date, and `{n}` is `{ty}`")).note(tr!(
            "文と明細には、長さと行の数を `count` で書きます。",
            "A text and a `rows` field take `count`: the length, the number of rows."
        )));
    }
    if let Some(c) = &f.count
        && !(is_text(f) || f.ty == TypeExpr::Rows)
    {
        out.push(s.err("E009", c.span, tr!("`count` は文と明細に書きます。`{n}` は `{ty}` です", "`count` is written on a text or a `rows` field, and `{n}` is `{ty}`")).note(tr!(
            "数、金額、日付には、値の範囲を `range` で書きます。",
            "A number, an amount and a date take `range`: the values they may be."
        )));
    }
    if let Some(r) = &f.round {
        if !is_num(f) {
            out.push(s.err("E009", r.span, tr!("`round` は数と金額に書きます。`{n}` は `{ty}` です", "`round` is written on a number or an amount, and `{n}` is `{ty}`")));
        } else if f.computed.is_none() {
            out.push(s.err("E009", r.span, tr!("`round` は計算するフィールドに書きます。`{n}` は申請者が入れます", "`round` is written on a computed field, and the requester fills in `{n}`")));
        } else if let TypeExpr::Num(u) = &f.ty {
            let mut scratch = Vec::new();
            let mut t = Typer::new(s, &mut scratch);
            let step = t.expr(&r.step);
            if !agree(&step, &Ty::Num(u.clone())) {
                let (w, ts, ws) = (written(s.src, r.step.span), step.shown(), ty.clone());
                out.push(s.err("E009", r.step.span, tr!("丸めの刻み `{w}` は `{ts}` で、`{n}` は `{ws}` です", "The step of the rounding, `{w}`, is `{ts}`, and `{n}` is `{ws}`")));
            }
        }
    }
    if let Some(c) = &f.open_in
        && f.ty != TypeExpr::Date
    {
        out.push(s.err("E009", c.span, tr!("`open in` は日付に書きます。`{n}` は `{ty}` です", "`open in` is written on a date, and `{n}` is `{ty}`")));
    }
    // the ends of a range: values of the field's type, the lower not above the upper
    if let Some(r) = &f.range
        && (is_num(f) || f.ty == TypeExpr::Date)
    {
        ends(s, f, r, &Ty::of_field(f), "range", out);
    }
    if let Some(c) = &f.count
        && (is_text(f) || f.ty == TypeExpr::Rows)
    {
        ends(s, f, c, &Ty::Num(ritsu_units::Unit::number()), "count", out);
    }
    // E101 and E102: what the requester fills in has its range and its limit
    let entered = f.computed.is_none();
    if (is_num(f) || f.ty == TypeExpr::Date) && entered {
        match &f.range {
            None => out.push(
                s.err("E101", f.name.span, tr!("`{n}` に値の範囲がありません", "`{n}` has no range"))
                    .note(range_note(f))
                    .fix_line(format!("{}  {}", line_of(s, f.line).trim_end(), range_example(f))),
            ),
            Some(r) if r.lo.is_none() || r.hi.is_none() => {
                let which = if r.lo.is_none() { tr!("下の端", "lower end") } else { tr!("上の端", "upper end") };
                out.push(s.err("E101", r.span, tr!("`{n}` の範囲に{}がありません", "The range of `{n}` has no {}", which.ja; which.en)).note(range_note(f)));
            }
            _ => {}
        }
    }
    if f.ty == TypeExpr::Rows && f.count.as_ref().is_none_or(|c| c.hi.is_none()) {
        let at = f.count.as_ref().map(|c| c.span).unwrap_or(f.name.span);
        out.push(s.err("E101", at, tr!("明細 `{n}` の行の数に上限がありません", "The rows `{n}` have no upper limit on their number")).note(tr!(
            "`count >=1 <=20` のように、行の数の上限を書いてください。上限は、合計の範囲と、生成するコードが受け付ける行の数になります。",
            "Write the most rows it takes, as in `count >=1 <=20`: the range of a sum over them, and the rows the generated code takes, come from it."
        )));
    }
    if is_text(f) && entered && f.count.as_ref().is_none_or(|c| c.hi.is_none()) {
        let at = f.count.as_ref().map(|c| c.span).unwrap_or(f.name.span);
        let mut d = s.err("E102", at, tr!("`{n}` の長さに上限がありません", "`{n}` has no upper limit on its length")).note(tr!(
            "`count <=100` のように、入れられる文字の数の上限を書いてください。上限が無いと、どれだけ長い文でも受け付けることになります。",
            "Write the most characters it takes, as in `count <=100`: with no limit, a text of any length is taken."
        ));
        if f.count.is_none() {
            d = d.fix_line(format!("{}  count <=100", line_of(s, f.line).trim_end()));
        }
        out.push(d);
    }
}

fn line_of<'s>(s: &'s Scope, line: usize) -> &'s str {
    s.src.lines().nth(line.saturating_sub(1)).unwrap_or("")
}

fn range_note(f: &Field) -> Text {
    let ex = range_example(f);
    tr!(
        "`{ex}` のように、入れられる値の範囲を書いてください。範囲は、画面の入力の検査と、経路の表の検査と、生成するコードの確かめに使います。",
        "Write the values it takes, as in `{ex}`: the screen checks the value against it, and the check of the route and the generated code take it."
    )
}

fn range_example(f: &Field) -> String {
    match &f.ty {
        TypeExpr::Date => "range >=2026-01-01 <=2027-12-31".into(),
        TypeExpr::Num(u) => match &u.dim {
            Dim::Money(_) => format!("range >=1{0} <=1_000_000{0}", u.unit),
            Dim::Rate => "range >=0% <=100%".into(),
            Dim::Number => "range >=1 <=20".into(),
            _ => format!("range >=1{0} <=100{0}", u.unit),
        },
        _ => "range >=1 <=20".into(),
    }
}

/// The ends of a `range` or a `count`: each a value of `want`'s type written in the file, the
/// lower not above the upper; a `count` counts in whole numbers from 0.
fn ends(s: &Scope, f: &Field, b: &Bounds, want: &Ty, word: &str, out: &mut Vec<Diag>) {
    let n = &f.name.text;
    let mut vals: Vec<Option<Rat>> = Vec::new();
    for e in [&b.lo, &b.hi].into_iter().flatten().map(|x| &x.value) {
        let w = written(s.src, e.span);
        let lit = match &e.kind {
            ExprKind::Num(x) => Some((Ty::Num(lit_unit(x)), Some(x.value))),
            ExprKind::Neg(inner) => match &inner.kind {
                ExprKind::Num(x) => Some((Ty::Num(lit_unit(x)), Some(Rat { num: -x.value.num, den: x.value.den }))),
                _ => None,
            },
            ExprKind::Date(d) => Some((Ty::Date, Some(Rat::int(*d as i128)))),
            _ => None,
        };
        let Some((t, v)) = lit else {
            out.push(s.err("E009", e.span, tr!("`{word}` の端には値を書きます。`{w}` は値ではありません", "The ends of a `{word}` are values written in the file, and `{w}` is not one")));
            vals.push(None);
            continue;
        };
        if !agree(&t, want) {
            let (ts, ws) = (t.shown(), want.shown());
            let what = if word == "count" { tr!("数（`count` は文字か行の数）", "a number (a `count` counts characters or rows)") } else { tr!("`{n}` の型", "the type of `{n}`") };
            out.push(s.err("E009", e.span, tr!("`{w}` は `{ts}` で、{}は `{ws}` です", "`{w}` is `{ts}`, and {} is `{ws}`", what.ja; what.en)));
            vals.push(None);
            continue;
        }
        if word == "count" && v.is_some_and(|v| !v.is_int() || v.num < 0) {
            out.push(s.err("E009", e.span, tr!("`count` の端は 0 以上の整数です。`{w}` はそうではありません", "The ends of a `count` are whole numbers from 0, and `{w}` is not one")));
            vals.push(None);
            continue;
        }
        // the ends of an amount in its unit (`1円` and `100銭` are one end)
        let v = match (&t, want, v) {
            (Ty::Num(from), Ty::Num(to), Some(v)) => from.convert(v, to).or(Some(v)),
            (_, _, v) => v,
        };
        vals.push(v);
    }
    if let (Some(lo), Some(hi)) = (&b.lo, &b.hi)
        && let [Some(a), Some(z)] = vals.as_slice()
    {
        let empty = match a.cmp_to(*z) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Equal => !(lo.inclusive && hi.inclusive),
            std::cmp::Ordering::Less => false,
        };
        if empty {
            let w = written(s.src, b.span);
            out.push(s.err("E009", b.span, tr!("`{w}` に入る値がありません。下の端が上の端を超えています", "No value is in `{w}`: the lower end is past the upper")));
        }
    }
}

// ── E104: how a computed amount rounds ──

/// Whether every value of the expression is a whole number of its unit (yen, grams, a count):
/// what a field of money, a quantity or a number holds, rulec's integers. A division, a rate that
/// is not a whole hundred percent, and a fraction written in the file may leave a fraction; a sum
/// and a difference of whole values, an entered value and a rounded one do not.
fn whole(s: &Scope, row: Option<&[Field]>, e: &Expr, seen: &mut BTreeSet<String>) -> bool {
    match &e.kind {
        ExprKind::Num(n) => n.unit != "%" && n.value.is_int(),
        ExprKind::Path(p) if p.len() == 1 => {
            let f = row.and_then(|r| r.iter().find(|f| f.name.text == p[0].text)).or_else(|| s.field(&p[0].text));
            match f {
                Some(f) => field_whole(s, row, f, seen),
                None => true,
            }
        }
        ExprKind::Sum(p) if p.len() == 2 => {
            let rows = s.field(&p[0].text);
            match rows.and_then(|r| r.children.iter().find(|c| c.name.text == p[1].text)) {
                Some(c) => field_whole(s, rows.map(|r| r.children.as_slice()), c, seen),
                None => true,
            }
        }
        ExprKind::Neg(x) => whole(s, row, x, seen),
        ExprKind::Bin(BinOp::Add | BinOp::Sub, a, b) => whole(s, row, a, seen) && whole(s, row, b, seen),
        ExprKind::Bin(BinOp::Mul, a, b) => factor_whole(s, row, a, seen) && factor_whole(s, row, b, seen),
        ExprKind::Bin(BinOp::Div, ..) => false,
        _ => true,
    }
}

/// A factor of a product: a rate is whole only as a whole hundred percent (`200%`), a rate field
/// never.
fn factor_whole(s: &Scope, row: Option<&[Field]>, e: &Expr, seen: &mut BTreeSet<String>) -> bool {
    match &e.kind {
        ExprKind::Num(n) if n.unit == "%" => n.value.div(Rat::int(100)).is_int(),
        _ => whole(s, row, e, seen),
    }
}

fn field_whole(s: &Scope, row: Option<&[Field]>, f: &Field, seen: &mut BTreeSet<String>) -> bool {
    let TypeExpr::Num(u) = &f.ty else { return true };
    if u.dim == Dim::Rate {
        return false;
    }
    match &f.computed {
        None => true,
        Some(_) if f.round.is_some() => true,
        Some(e) => {
            // a ring of computed fields is lowered and said in a later stage; here it is whole
            if !seen.insert(f.name.text.clone()) {
                return true;
            }
            whole(s, row, e, seen)
        }
    }
}

fn rounding(s: &Scope, out: &mut Vec<Diag>) {
    let mut each = |f: &Field, row: Option<&[Field]>| {
        let (Some(e), TypeExpr::Num(u)) = (&f.computed, &f.ty) else { return };
        if f.round.is_some() || u.dim == Dim::Rate || u.compares_only() {
            return;
        }
        let mut seen = BTreeSet::from([f.name.text.clone()]);
        if whole(s, row, e, &mut seen) {
            return;
        }
        let (n, w) = (&f.name.text, written(s.src, e.span));
        let step = format!("round down(1{})", u.unit);
        out.push(
            s.err("E104", e.span, tr!("`{n}` の計算に丸め方がありません", "`{n}` is computed with no rounding"))
                .note(tr!(
                    "`{w}` は端数が出ることがあります。`{step}` のように、丸め方と刻みを書いてください（`up`、`down`、`half_up`、`half_down`、`half_even`）。",
                    "`{w}` can leave a fraction: say how it rounds and to what, as in `{step}` (`up`, `down`, `half_up`, `half_down`, `half_even`)."
                ))
                .fix_line(format!("{}  {step}", line_of(s, f.line).trim_end())),
        );
    };
    for f in s.fields() {
        each(f, None);
        for c in &f.children {
            each(c, Some(&f.children));
        }
    }
}

// ── E106: what is decided when the request is made reads what is filled in later ──

/// The fields that are empty when the request is made: the ones approvers fill in, and the ones
/// computed from those.
fn late_fields(s: &Scope) -> BTreeSet<String> {
    let mut late: BTreeSet<String> = s.fields().iter().filter(|f| f.approvers_only.is_some()).map(|f| f.name.text.clone()).collect();
    loop {
        let more: Vec<String> = s
            .fields()
            .iter()
            .filter(|f| !late.contains(&f.name.text))
            .filter(|f| f.computed.as_ref().is_some_and(|e| e.paths().iter().any(|p| late.contains(&p[0].text))))
            .map(|f| f.name.text.clone())
            .collect();
        if more.is_empty() {
            return late;
        }
        late.extend(more);
    }
}

fn late_read(s: &Scope, late: &BTreeSet<String>, at: &Word, out: &mut Vec<Diag>) {
    let n = &at.text;
    let why = match s.field(n) {
        Some(f) if f.approvers_only.is_some() => tr!("`{n}` は承認者だけが入れるフィールド（`approvers only`）なので", "`{n}` is filled in by approvers (`approvers only`), so it"),
        _ => tr!("`{n}` は承認者だけが入れるフィールドから計算するので", "`{n}` is computed from a field approvers fill in, so it"),
    };
    let _ = late;
    out.push(s.err("E106", at.span, tr!("申請のときに決める条件が、まだ入っていない `{n}` を読みます", "What is decided when the request is made reads `{n}`, which is not filled in yet")).note(tr!(
        "{}、申請のときには空です。申請のときに決める条件（表示と必須の条件、経路、ステップの条件、`on submitted`）は、申請者が入れるフィールドだけを読むようにしてください。",
        "{} is empty when the request is made. What is decided then (when a field is shown or required, the route, the conditions of the stages, `on submitted`) reads only fields the requester fills in.",
        why.ja;
        why.en
    )));
}

fn late_expr(s: &Scope, late: &BTreeSet<String>, e: &Expr, out: &mut Vec<Diag>) {
    for p in e.paths() {
        if late.contains(&p[0].text) && s.field(&p[0].text).is_some() {
            late_read(s, late, &p[0], out);
        }
    }
}

fn late(s: &Scope, out: &mut Vec<Diag>) {
    let late = late_fields(s);
    if late.is_empty() {
        return;
    }
    let f = s.file;
    // the conditions of the fields the requester fills in
    for fd in s.fields().iter().filter(|x| x.approvers_only.is_none()) {
        for c in [&fd.shown_when, &fd.required_when].into_iter().flatten() {
            late_expr(s, &late, c, out);
        }
    }
    // the route, decided when the request is made
    match &f.route {
        Some(Route::Table(t)) => {
            for col in &t.inputs {
                if col.len() == 1 && late.contains(&col[0].text) {
                    late_read(s, &late, &col[0], out);
                }
            }
        }
        Some(Route::Stages(lines)) => {
            for l in lines {
                if let Some(w) = &l.when {
                    late_expr(s, &late, w, out);
                }
            }
        }
        Some(Route::Shared { args, .. }) => {
            for a in args {
                late_expr(s, &late, &a.value, out);
            }
        }
        None => {}
    }
    // what is done when the request is made
    if let Some(t) = &f.ticket {
        for l in &t.lines {
            if let TicketLine::On { endings, call, when, .. } = l
                && endings.iter().any(|(e, _)| *e == Ending::Submitted)
            {
                for a in &call.args {
                    late_expr(s, &late, &a.value, out);
                }
                if let Some(w) = when {
                    late_expr(s, &late, w, out);
                }
            }
        }
    }
}

// ── W108: a field approvers may edit that the route reads ──

/// The fields the route reads: its table's inputs, its stages' conditions, what a shared route is
/// given, the field a `lookup` looks up by and the field the requester chooses a person in; and
/// every field a field it reads is computed from. Each with the field the route reads it through:
/// itself, or the computed field it reads.
fn route_reads(s: &Scope) -> BTreeMap<String, String> {
    let f = s.file;
    let mut direct: Vec<String> = Vec::new();
    match &f.route {
        Some(Route::Table(t)) => direct.extend(t.inputs.iter().filter(|c| c.len() == 1).map(|c| c[0].text.clone())),
        Some(Route::Stages(lines)) => {
            for l in lines {
                if let Some(w) = &l.when {
                    direct.extend(w.paths().iter().map(|p| p[0].text.clone()));
                }
            }
        }
        Some(Route::Shared { args, .. }) => {
            for a in args {
                direct.extend(a.value.paths().iter().map(|p| p[0].text.clone()));
            }
        }
        None => {}
    }
    for w in &f.who {
        match &w.how {
            How::Lookup { by, .. } => direct.push(by.text.clone()),
            How::ChosenBy { previous: false, field } => direct.push(field.text.clone()),
            _ => {}
        }
    }
    let mut read: BTreeMap<String, String> = BTreeMap::new();
    for n in direct {
        read.entry(n.clone()).or_insert(n);
    }
    loop {
        let more: Vec<(String, String)> = s
            .fields()
            .iter()
            .filter_map(|x| Some((x.computed.as_ref()?, read.get(&x.name.text)?.clone())))
            .flat_map(|(e, via)| e.paths().iter().map(|p| (p[0].text.clone(), via.clone())).collect::<Vec<_>>())
            .filter(|(n, _)| !read.contains_key(n) && s.field(n).is_some())
            .collect();
        if more.is_empty() {
            break;
        }
        for (n, via) in more {
            read.entry(n).or_insert(via);
        }
    }
    read.retain(|n, _| s.field(n).is_some());
    read
}

fn edits(s: &Scope, out: &mut Vec<Diag>) {
    let Some(t) = &s.file.ticket else { return };
    let read = route_reads(s);
    for l in &t.lines {
        if let TicketLine::ApproversEdit { fields, .. } = l {
            for w in fields {
                let Some(via) = read.get(&w.text) else { continue };
                let n = &w.text;
                let reads = if via == n {
                    tr!("経路は `{n}` を読みます。", "The route reads `{n}`.")
                } else {
                    tr!("経路は `{via}` を読み、`{via}` は `{n}` から計算します。", "The route reads `{via}`, which is computed from `{n}`.")
                };
                out.push(s.warn("W108", w.span, tr!("承認者が `{n}` を直すと、経路が変わることがあります", "When an approver edits `{n}`, the route may change")).note(tr!(
                    "{}承認者が直したときは、今のステップより後の経路を直した値で決め直し、前のステップの答えは変えません。それでよいか確かめてください。",
                    "{} When an approver edits it, the route after the stage at hand is decided again with the new value, and the answers of the stages before it stay. See that this is what is meant.",
                    reads.ja;
                    reads.en
                )));
            }
        }
    }
}

// ── W109: a field that asks for a secret ──

/// The words that name a secret, in English (a word of the name or of the title) and in Japanese
/// (anywhere in it).
pub const SECRET_WORDS: [&str; 12] = ["password", "passwd", "passphrase", "passcode", "pin", "secret", "token", "apikey", "credential", "credentials", "cvv", "cvc"];
pub const SECRET_PAIRS: [(&str, &str); 2] = [("api", "key"), ("private", "key")];
pub const SECRET_JA: [&str; 6] = ["パスワード", "暗証番号", "秘密鍵", "シークレット", "トークン", "認証情報"];

/// The secret a name or a title names, if any.
fn secret_word(text: &str) -> Option<String> {
    if let Some(w) = SECRET_JA.iter().find(|w| text.contains(*w)) {
        return Some(w.to_string());
    }
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    if let Some(w) = words.iter().find(|w| SECRET_WORDS.contains(*w)) {
        return Some(w.to_string());
    }
    words.windows(2).find(|p| SECRET_PAIRS.contains(&(p[0], p[1]))).map(|p| format!("{} {}", p[0], p[1]))
}

fn secrets(s: &Scope, out: &mut Vec<Diag>) {
    let mut each = |f: &Field| {
        if !is_text(f) {
            return;
        }
        let found = [Some(f.name.text.as_str()), f.name.alias.as_ref().map(|a| a.text.as_str()), f.title.as_deref()].into_iter().flatten().find_map(secret_word);
        let Some(w) = found else { return };
        let n = &f.name.text;
        out.push(s.warn("W109", f.name.span, tr!("`{n}` は秘密の値（{w}）を入れさせるフィールドです", "`{n}` asks the requester for a secret ({w})")).note(tr!(
            "入れた値はチケットの履歴に残り、承認者と、ページとログを読める人に見えます。秘密の値は申請で渡さず、それを扱う仕組み（鍵の置き場、本人が入れる画面）で渡してください。",
            "What is filled in stays in the ticket's history, where approvers and whoever reads the pages and the logs see it. Do not pass a secret through a request: pass it through what is made for it (a secret store, a screen the person fills in themselves)."
        )));
    };
    for f in s.fields() {
        each(f);
        for c in &f.children {
            each(c);
        }
    }
}
