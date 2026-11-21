//! The type of every expression and condition (DESIGN 4.5), in rulec's types: a number in its unit
//! (`number`, `money[JPY]`, `rate`, `mass[kg]`), a date, a text, a bool, a choice, a person, a
//! team, an attachment, the rows of a `rows` field. What an expression reads is a field, a value of
//! a `choice` it is compared with, `requester` (`requester.id`, `requester.<kind of team>`),
//! `ticket` (`ticket.id`, `ticket.round`), a rule's output (read in a later stage, through the
//! ports), or a master's field (`lookup m.vendors.name by vendor`).
//!
//! A name an expression reads that is not declared is E004; a value of another type than the place
//! it is written in takes is E009: a comparison of two types, a sum of money and a date, a
//! condition that is not true or false, a cell of the route that is not a value of its column.

use crate::ast::*;
use crate::diag::Diag;
use crate::lex::Num;
use crate::names::{self, Scope};
use ritsu_base::text::Text;
use ritsu_units::{Dim, Unit};

/// The type of a value.
#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Num(Unit),
    Date,
    Text,
    Bool,
    /// A `choice`, with the values written under it.
    Choice(Vec<String>),
    /// One of a master's rows: which keys it takes is read from the master's data, where a value is
    /// written for it ([`crate::route`]). Where a value is written for a field of a rule's enum, the
    /// field takes the enum's values, as a `choice` ([`Typer::value_ty`]).
    ChoiceFrom,
    Person,
    Team,
    Attachment,
    Rows,
    None,
    /// A number of business days, which moves a date.
    Days,
    /// What is read in a later stage (a rule's output), or what is wrong and said already.
    Unknown,
}

/// The type of a rule's column as ringi reads it: an optional one (`T?`) as `T`, which may be none.
pub fn column_ty(facts: &ritsu_ports::RuleFacts, ty: &ritsu_ports::ColumnType) -> Ty {
    use ritsu_ports::ColumnType;
    match ty {
        ColumnType::Bool => Ty::Bool,
        ColumnType::Str => Ty::Text,
        ColumnType::Date { .. } => Ty::Date,
        ColumnType::Enum { name, .. } => Ty::Choice(facts.enums.iter().find(|e| &e.name == name).map(|e| e.values.iter().map(|v| v.public.clone()).collect()).unwrap_or_default()),
        ColumnType::Num { unit, .. } => Ty::Num(unit.clone().unwrap_or_else(Unit::number)),
        ColumnType::Opt(inner) => column_ty(facts, inner),
    }
}

impl Ty {
    pub fn of_field(f: &Field) -> Ty {
        match &f.ty {
            TypeExpr::Text | TypeExpr::LongText => Ty::Text,
            TypeExpr::Num(u) => Ty::Num(u.clone()),
            TypeExpr::Date => Ty::Date,
            TypeExpr::Choice => Ty::Choice(f.options.iter().map(|o| o.name.text.clone()).collect()),
            TypeExpr::ChoiceFrom(_) => Ty::ChoiceFrom,
            TypeExpr::Attachment => Ty::Attachment,
            TypeExpr::Person => Ty::Person,
            TypeExpr::Team => Ty::Team,
            TypeExpr::Bool => Ty::Bool,
            TypeExpr::Rows => Ty::Rows,
        }
    }

    /// The type as the language writes it.
    pub fn shown(&self) -> String {
        match self {
            Ty::Num(u) => unit_type(u),
            Ty::Date => "date".into(),
            Ty::Text => "text".into(),
            Ty::Bool => "bool".into(),
            Ty::Choice(_) => "choice".into(),
            Ty::ChoiceFrom => "choice from …".into(),
            Ty::Person => "person".into(),
            Ty::Team => "team".into(),
            Ty::Attachment => "attachment".into(),
            Ty::Rows => "rows".into(),
            Ty::None => "none".into(),
            Ty::Days => "business days".into(),
            Ty::Unknown => "?".into(),
        }
    }

    /// Whether a value of the type is ordered (`<`, `>=`, a `range`): a number in its unit, a date.
    pub fn ordered(&self) -> bool {
        matches!(self, Ty::Num(_) | Ty::Date | Ty::Unknown)
    }
}

/// The unit of a number as written: `30_000JPY` is money in yen, `60%` a rate, `5` a number.
pub fn lit_unit(n: &Num) -> Unit {
    match n.unit.as_str() {
        "" => Unit::number(),
        "%" => Unit::rate(None),
        u => match ritsu_units::table::unit(u) {
            Some((Dim::Money(_), _)) => Unit::money(u, None).unwrap_or_else(Unit::number),
            Some((dim, _)) => Unit { dim, unit: u.to_string(), tax: None, step: None },
            None => Unit::number(),
        },
    }
}

/// Whether a value of type `a` fits where `b` is taken (and the other way round: the check is the
/// same both ways). A number fits a number of its dimension (a currency is one), whatever its tax
/// and its spelling (`JPY` and `円`); `none` fits anything (a field that may be empty); a text fits
/// a choice from a master, whose rows a key of text names.
pub fn agree(a: &Ty, b: &Ty) -> bool {
    match (a, b) {
        (Ty::Unknown, _) | (_, Ty::Unknown) | (Ty::None, _) | (_, Ty::None) => true,
        (Ty::Num(x), Ty::Num(y)) => x.dim == y.dim,
        (Ty::Choice(_), Ty::Choice(_)) | (Ty::ChoiceFrom, Ty::ChoiceFrom) => true,
        (Ty::ChoiceFrom, Ty::Text) | (Ty::Text, Ty::ChoiceFrom) => true,
        _ => a == b,
    }
}

/// The text of an expression as it is written.
pub fn written(src: &str, span: Span) -> String {
    let Some(line) = src.lines().nth(span.line.saturating_sub(1)) else { return String::new() };
    let cs: Vec<char> = line.chars().collect();
    let from = span.col.saturating_sub(1).min(cs.len());
    let to = if span.end > span.col { (span.end - 1).min(cs.len()) } else { cs.len() };
    cs[from..to.max(from)].iter().collect::<String>().trim().to_string()
}

/// The checker of the expressions of one file: the place to look names up, where to say what is
/// wrong, and the fields of the row a computed field of a `rows` field is in.
pub struct Typer<'s, 'a> {
    pub s: &'s Scope<'a>,
    pub out: &'s mut Vec<Diag>,
    pub row: Option<&'a [Field]>,
}

impl<'s, 'a> Typer<'s, 'a> {
    pub fn new(s: &'s Scope<'a>, out: &'s mut Vec<Diag>) -> Typer<'s, 'a> {
        Typer { s, out, row: None }
    }

    fn e009(&mut self, at: Span, msg: Text) {
        let d = self.s.err("E009", at, msg);
        self.out.push(d);
    }

    fn text(&self, e: &Expr) -> String {
        written(self.s.src, e.span)
    }

    fn field_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.row.map(|r| r.iter().map(|f| f.name.text.clone()).collect()).unwrap_or_default();
        v.extend(self.s.fields().iter().map(|f| f.name.text.clone()));
        v
    }

    /// A field by its name: a field of the row first, then of the form.
    fn find_field(&self, name: &str) -> Option<&'a Field> {
        self.row.and_then(|r| r.iter().find(|f| f.name.text == name)).or_else(|| self.s.field(name))
    }

    /// The type of an expression.
    pub fn expr(&mut self, e: &Expr) -> Ty {
        match &e.kind {
            ExprKind::Num(n) => Ty::Num(lit_unit(n)),
            ExprKind::Date(_) => Ty::Date,
            ExprKind::Str(_) => Ty::Text,
            ExprKind::None => Ty::None,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Path(p) => self.path(p),
            ExprKind::Sum(p) => self.sum(p, e.span),
            ExprKind::Lookup(p, by) => self.lookup(p, by),
            ExprKind::BusinessDays(n, cal) => {
                let t = self.expr(n);
                if !agree(&t, &Ty::Num(Unit::number())) || matches!(t, Ty::None) {
                    let (x, ts) = (self.text(n), t.shown());
                    self.e009(n.span, tr!("営業日の数は `number` です。`{x}` は `{ts}` です", "A number of business days is a `number`, and `{x}` is `{ts}`"));
                }
                names::calendar_ref(self.s, cal.as_ref(), e.span, self.out);
                Ty::Days
            }
            ExprKind::Neg(x) => {
                let t = self.expr(x);
                match t {
                    Ty::Num(_) | Ty::Unknown => t,
                    _ => {
                        let (w, ts) = (self.text(x), t.shown());
                        self.e009(e.span, tr!("`{w}` は `{ts}` なので、負にできません", "`{w}` is `{ts}`, which has no negative"));
                        Ty::Unknown
                    }
                }
            }
            ExprKind::Bin(op, a, b) => {
                let (ta, tb) = (self.expr(a), self.expr(b));
                self.arith(*op, e, a, b, ta, tb)
            }
            ExprKind::Cmp(op, a, b) => {
                self.compare(*op, a, b);
                Ty::Bool
            }
            ExprKind::And(a, b) | ExprKind::Or(a, b) => {
                self.cond(a);
                self.cond(b);
                Ty::Bool
            }
            ExprKind::Not(a) => {
                self.cond(a);
                Ty::Bool
            }
            ExprKind::Person(_) => Ty::Person,
            ExprKind::Team(_) => Ty::Team,
            ExprKind::List(_) | ExprKind::Record(_) => {
                self.e009(e.span, tr!("リストとレコードは、シナリオの `form` で明細の行を書くときだけ使えます", "A list or a record is written only for the rows of a `rows` field in a scenario's `form`"));
                Ty::Unknown
            }
        }
    }

    /// A condition: true or false.
    pub fn cond(&mut self, e: &Expr) {
        let t = self.expr(e);
        if !matches!(t, Ty::Bool | Ty::Unknown) {
            let (w, ts) = (self.text(e), t.shown());
            self.e009(e.span, tr!("条件は真か偽ですが、`{w}` は `{ts}` です", "A condition is true or false, and `{w}` is `{ts}`"));
        }
    }

    fn path(&mut self, p: &[Word]) -> Ty {
        let first = &p[0].text;
        if let Some(f) = self.find_field(first) {
            if p.len() == 1 {
                return Ty::of_field(f);
            }
            let whole = dotted(p);
            if f.ty == TypeExpr::Rows {
                let n = &f.name.text;
                self.e009(p[0].span, tr!("`{whole}` は明細 `{n}` の列です。合計するなら `sum({whole})` と書きます", "`{whole}` is a column of the rows `{n}`: to add it up, write `sum({whole})`"));
            } else {
                let (n, x) = (&f.name.text, &p[1].text);
                self.out.push(self.s.err("E004", p[1].span, tr!("`{n}` に `{x}` はありません", "`{n}` has no `{x}`")));
            }
            return Ty::Unknown;
        }
        match first.as_str() {
            "requester" => match p.len() {
                1 => Ty::Person,
                2 if p[1].text == "id" => Ty::Text,
                2 => {
                    let k = &p[1];
                    match self.s.org() {
                        None => {
                            let n = &k.text;
                            self.out.push(self.s.err("E004", k.span, tr!("`requester.{n}` は組織を読みますが、このファイルに `use organization` がありません", "`requester.{n}` reads the organization, and the file has no `use organization`")));
                        }
                        Some(_) => names::kind_ref(self.s, k, self.out),
                    }
                    Ty::Team
                }
                _ => {
                    let whole = dotted(p);
                    self.out.push(self.s.err("E004", p[2].span, tr!("`{whole}` は読めません。`requester` が持つのは `id` と、組織のチームの種類（`requester.department`）です", "`{whole}` cannot be read: `requester` has `id` and the kinds of team of the organization (`requester.department`)")));
                    Ty::Unknown
                }
            },
            "ticket" => match (p.len(), p.get(1).map(|w| w.text.as_str())) {
                (2, Some("id")) => Ty::Text,
                (2, Some("round")) => Ty::Num(Unit::number()),
                (2, Some("attempt")) => Ty::Text,
                _ => {
                    let whole = dotted(p);
                    self.out.push(self.s.err("E004", p[0].span, tr!("`{whole}` は読めません。`ticket` が持つのは `ticket.id`、`ticket.round`、`ticket.attempt` です", "`{whole}` cannot be read: `ticket` has `ticket.id`, `ticket.round` and `ticket.attempt`")));
                    Ty::Unknown
                }
            },
            _ => match self.s.alias(first).map(|(u, _)| u.kind) {
                // a rule's output, of the type rulec says (DESIGN 6.6); a dates file's date is read
                // by the module a request is lowered to (stage D)
                Some(UseKind::Rule) => self.rule_output(p),
                Some(UseKind::Dates) => Ty::Unknown,
                Some(k) => {
                    let w = k.word();
                    self.out.push(self.s.err("E004", p[0].span, tr!("`{first}` は `use {w}` の別名なので、式では読めません", "`{first}` is the alias of a `use {w}`, which an expression does not read")));
                    Ty::Unknown
                }
                None => {
                    let names = self.field_names();
                    self.out.push(names::undeclared(self.s, p[0].span, tr!("`{first}` は宣言されていません", "`{first}` is not declared"), tr!("フィールド", "fields"), &names));
                    Ty::Unknown
                }
            },
        }
    }

    /// `authority.board`: an output of a rule a `use rule` reads, of the type rulec gives it. E004
    /// for an output the rule does not have; nothing for a rule that could not be read (said at its
    /// `use` line).
    fn rule_output(&mut self, p: &[Word]) -> Ty {
        let Some(facts) = self.s.rule_facts(&p[0].text) else { return Ty::Unknown };
        let alias = &p[0].text;
        let Some(o) = p.get(1).filter(|_| p.len() == 2).and_then(|w| facts.outputs.iter().find(|o| o.name == w.text || o.alias == w.text)) else {
            let whole = dotted(p);
            let names: Vec<String> = facts.outputs.iter().map(|o| o.name.clone()).collect();
            self.out.push(names::undeclared(self.s, p.get(1).unwrap_or(&p[0]).span, tr!("規則 `{alias}` に、出力 `{whole}` はありません", "The rule `{alias}` has no output `{whole}`"), tr!("規則の出力", "outputs of the rule"), &names));
            return Ty::Unknown;
        };
        column_ty(facts, &o.ty)
    }

    /// `sum(items.price)`: a column of numbers of a `rows` field.
    fn sum(&mut self, p: &[Word], at: Span) -> Ty {
        let whole = dotted(p);
        let rows = (p.len() == 2).then(|| self.s.field(&p[0].text)).flatten().filter(|f| f.ty == TypeExpr::Rows);
        let Some(rows) = rows else {
            if p.len() == 1 && self.find_field(&p[0].text).is_none() {
                self.path(p);
                return Ty::Unknown;
            }
            self.out.push(self.s.err("E004", at, tr!("`sum` は明細の列を合計します。`{whole}` は明細の列ではありません", "`sum` adds up a column of a `rows` field, which `{whole}` is not")));
            return Ty::Unknown;
        };
        let Some(col) = rows.children.iter().find(|c| c.name.text == p[1].text) else {
            let (n, x) = (&rows.name.text, &p[1].text);
            let names: Vec<String> = rows.children.iter().map(|c| c.name.text.clone()).collect();
            self.out.push(names::undeclared(self.s, p[1].span, tr!("明細 `{n}` に `{x}` という列はありません", "The rows `{n}` have no column `{x}`"), tr!("列", "columns"), &names));
            return Ty::Unknown;
        };
        match Ty::of_field(col) {
            t @ Ty::Num(_) => t,
            t => {
                let ts = t.shown();
                self.e009(at, tr!("`{whole}` は `{ts}` なので、合計できません", "`{whole}` is `{ts}`, which cannot be added up"));
                Ty::Unknown
            }
        }
    }

    /// `lookup m.vendors.name by vendor`: a field of the row of a master a field chose.
    pub fn lookup(&mut self, p: &[Word], by: &Word) -> Ty {
        let mut t = Ty::Unknown;
        if p.len() != 3 {
            let whole = dotted(p);
            self.out.push(self.s.err("E004", p[0].span, tr!("`{whole}` はマスタのフィールドではありません。`m.systems.owner` のように書いてください", "`{whole}` is no field of a master: write it as `m.systems.owner`")));
        } else if let Some(m) = names::master(self.s, &p[0], &p[1], self.out) {
            match m.fields.iter().find(|x| x.name.text == p[2].text) {
                Some(f) => t = Ty::of_field(f),
                None => {
                    let (n, mn) = (&p[2].text, &m.name.text);
                    let names: Vec<String> = m.fields.iter().map(|x| x.name.text.clone()).collect();
                    self.out.push(names::undeclared(self.s, p[2].span, tr!("マスタ `{mn}` に `{n}` というフィールドはありません", "The master `{mn}` has no field `{n}`"), tr!("フィールド", "fields"), &names));
                }
            }
        }
        match self.s.field(&by.text) {
            None => {
                let n = &by.text;
                let names = self.field_names();
                self.out.push(names::undeclared(self.s, by.span, tr!("`{n}` というフィールドはありません", "There is no field `{n}`"), tr!("フィールド", "fields"), &names));
            }
            Some(f) => {
                let from = match &f.ty {
                    TypeExpr::ChoiceFrom(q) => Some(dotted(q)),
                    _ => None,
                };
                let want = if p.len() == 3 { Some(dotted(&p[..2])) } else { None };
                if let Some(w) = want
                    && from.as_deref() != Some(w.as_str())
                {
                    let (n, ts) = (&f.name.text, f.ty.word());
                    self.e009(by.span, tr!("`lookup` は `choice from {w}` のフィールドで引きます。`{n}` は `{ts}` です", "`lookup` looks up by a field of `choice from {w}`, and `{n}` is `{ts}`"));
                }
            }
        }
        t
    }

    fn arith(&mut self, op: BinOp, e: &Expr, a: &Expr, b: &Expr, ta: Ty, tb: Ty) -> Ty {
        use BinOp::*;
        if matches!(ta, Ty::Unknown) || matches!(tb, Ty::Unknown) {
            return match (op, &ta, &tb) {
                (Add | Sub, Ty::Date, _) | (Add, _, Ty::Date) => Ty::Date,
                _ => Ty::Unknown,
            };
        }
        let out = match (op, &ta, &tb) {
            (Add | Sub, Ty::Num(x), Ty::Num(y)) if x.dim == y.dim && !x.compares_only() => Some(ta.clone()),
            (Add | Sub, Ty::Date, Ty::Days) | (Add, Ty::Days, Ty::Date) => Some(Ty::Date),
            (Sub, Ty::Date, Ty::Date) => Some(Ty::Num(Unit::number())),
            (Mul, Ty::Num(x), Ty::Num(y)) if !x.compares_only() && !y.compares_only() => match (&x.dim, &y.dim) {
                // a rate of a count is a rate, as rulec types it (`share * 2`)
                (Dim::Rate, Dim::Number) => Some(ta.clone()),
                (Dim::Number, _) | (Dim::Rate, _) => Some(tb.clone()),
                (_, Dim::Number) | (_, Dim::Rate) => Some(ta.clone()),
                _ => None,
            },
            (Div, Ty::Num(x), Ty::Num(y)) if !x.compares_only() && !y.compares_only() => match &y.dim {
                Dim::Number | Dim::Rate => Some(ta.clone()),
                d if *d == x.dim => Some(Ty::Num(Unit::rate(None))),
                _ => None,
            },
            _ => None,
        };
        match out {
            Some(t) => t,
            None => {
                let (whole, xa, xb, sa, sb) = (self.text(e), self.text(a), self.text(b), ta.shown(), tb.shown());
                self.e009(e.span, tr!("`{whole}` は計算できません。`{xa}` は `{sa}`、`{xb}` は `{sb}` です", "`{whole}` cannot be computed: `{xa}` is `{sa}` and `{xb}` is `{sb}`"));
                Ty::Unknown
            }
        }
    }

    /// A value that may be the name of a value of a `choice` (`category is travel`): a name that is
    /// one of its values, and no field of its own, is that value.
    pub fn against(&mut self, e: &Expr, want: &Ty) -> Ty {
        if let ExprKind::Path(p) = &e.kind
            && p.len() == 1
            && self.find_field(&p[0].text).is_none()
        {
            match want {
                Ty::Choice(values) => {
                    if values.contains(&p[0].text) {
                        return want.clone();
                    }
                    let n = &p[0].text;
                    let d = names::undeclared(self.s, p[0].span, tr!("`{n}` は選べる値ではありません", "`{n}` is not one of the values to choose from"), tr!("値", "values"), values);
                    self.out.push(d);
                    return Ty::Unknown;
                }
                // a row of a master (which keys there are is read from its data, where a value is
                // written for the field), or a value of a rule that could not be read
                Ty::ChoiceFrom if self.s.alias(&p[0].text).is_none() && !matches!(p[0].text.as_str(), "requester" | "ticket") => return Ty::ChoiceFrom,
                _ => {}
            }
        }
        self.expr(e)
    }

    /// The type of the values written for a field: a `choice from` a rule's enum takes the enum's
    /// values, read from the rule, as a `choice` takes its own; any other field its type.
    pub fn value_ty(&self, f: &Field) -> Ty {
        if let TypeExpr::ChoiceFrom(p) = &f.ty
            && p.len() == 2
            && let Some(facts) = self.s.rule_facts(&p[0].text)
            && let Some(e) = facts.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text)
        {
            return Ty::Choice(e.values.iter().map(|v| v.public.clone()).collect());
        }
        Ty::of_field(f)
    }

    /// The values of a rule's enum, when the expression is a field that chooses one of them.
    fn enum_field(&self, e: &Expr) -> Option<Ty> {
        let ExprKind::Path(p) = &e.kind else { return None };
        let f = self.find_field(&p.first()?.text).filter(|_| p.len() == 1)?;
        Some(self.value_ty(f)).filter(|t| matches!(f.ty, TypeExpr::ChoiceFrom(_)) && matches!(t, Ty::Choice(_)))
    }

    fn compare(&mut self, op: CmpOp, a: &Expr, b: &Expr) {
        let ta = match self.enum_field(a) {
            Some(t) => t,
            None => self.expr(a),
        };
        let tb = match self.enum_field(b) {
            Some(t) => t,
            None => self.against(b, &ta),
        };
        if !agree(&ta, &tb) {
            let (xa, xb, sa, sb) = (self.text(a), self.text(b), ta.shown(), tb.shown());
            self.e009(a.span, tr!("`{xa}` と `{xb}` は比べられません（`{sa}` と `{sb}`）", "`{xa}` and `{xb}` cannot be compared (`{sa}` and `{sb}`)"));
            return;
        }
        if !matches!(op, CmpOp::Is | CmpOp::IsNot) {
            for (x, t) in [(a, &ta), (b, &tb)] {
                if !t.ordered() {
                    let (w, ts) = (self.text(x), t.shown());
                    self.e009(x.span, tr!("`{w}` は `{ts}` なので、大きさを比べられません", "`{w}` is `{ts}`, which has no order to compare by"));
                    return;
                }
            }
        }
    }

    /// E009 when the value written does not fit where it is written (`what`, of type `want`).
    pub fn fits(&mut self, e: &Expr, want: &Ty, what: &Text) {
        let t = self.against(e, want);
        if !agree(&t, want) {
            let (w, ts, ws) = (self.text(e), t.shown(), want.shown());
            self.e009(e.span, tr!("`{w}` は `{ts}` で、{}は `{ws}` です", "`{w}` is `{ts}`, and {} is `{ws}`", what.ja; what.en));
        }
    }

    /// A value of a scenario's form, for its field: a `rows` field takes a list of records, each of
    /// the fields of its row.
    pub fn field_value(&mut self, e: &Expr, f: &'a Field) {
        let n = &f.name.text;
        // an attachment is given by the name of its file
        if f.ty == TypeExpr::Attachment && matches!(e.kind, ExprKind::Str(_) | ExprKind::None) {
            return;
        }
        if f.ty != TypeExpr::Rows {
            let want = self.value_ty(f);
            self.fits(e, &want, &tr!("`{n}`", "`{n}`"));
            return;
        }
        let ExprKind::List(rows) = &e.kind else {
            let w = self.text(e);
            self.e009(e.span, tr!("明細 `{n}` には、行のリスト（`[{{…}}, {{…}}]`）を書きます。`{w}` はそうではありません", "The rows `{n}` take a list of rows (`[{{…}}, {{…}}]`), which `{w}` is not"));
            return;
        };
        for r in rows {
            let ExprKind::Record(cells) = &r.kind else {
                let w = self.text(r);
                self.e009(r.span, tr!("明細の行は `{{what: …, price: …}}` のように書きます。`{w}` はそうではありません", "A row is written as `{{what: …, price: …}}`, which `{w}` is not"));
                continue;
            };
            for (k, v) in cells {
                match f.children.iter().find(|c| c.name.text == k.text) {
                    Some(c) if c.ty == TypeExpr::Attachment && matches!(v.kind, ExprKind::Str(_) | ExprKind::None) => {}
                    Some(c) => {
                        let (cn, want) = (&c.name.text, self.value_ty(c));
                        self.fits(v, &want, &tr!("`{n}.{cn}`", "`{n}.{cn}`"));
                    }
                    None => {
                        let x = &k.text;
                        let names: Vec<String> = f.children.iter().map(|c| c.name.text.clone()).collect();
                        self.out.push(names::undeclared(self.s, k.span, tr!("明細 `{n}` に `{x}` という列はありません", "The rows `{n}` have no column `{x}`"), tr!("列", "columns"), &names));
                    }
                }
            }
        }
    }

    /// A cell of an input column of the route, the examples or a table: its test, for the column's
    /// type.
    fn test(&mut self, t: &Test, col: &Ty, name: &str) {
        let what = tr!("列 `{name}`", "the column `{name}`");
        match t {
            Test::Any => {}
            Test::Values { values, .. } => {
                for v in values {
                    self.fits(v, col, &what);
                }
            }
            Test::Compare(cmps) => {
                if !col.ordered() {
                    let ts = col.shown();
                    if let Some((_, v)) = cmps.first() {
                        self.e009(v.span, tr!("列 `{name}` は `{ts}` なので、大きさで比べられません", "The column `{name}` is `{ts}`, which has no order to compare by"));
                    }
                    return;
                }
                for (_, v) in cmps {
                    self.fits(v, col, &what);
                }
            }
        }
    }

    /// The type of an input column of the route or the examples: a field's, or a rule's output's.
    /// None for `requester…` and `ticket…`, which are no column (E309 says so where the route is
    /// lowered).
    fn column(&self, col: &[Word]) -> Option<Ty> {
        match self.s.field(&col[0].text) {
            Some(f) if col.len() == 1 => Some(self.value_ty(f)),
            Some(_) => Some(Ty::Unknown),
            None if matches!(col[0].text.as_str(), "requester" | "ticket") => None,
            None => {
                let facts = (col.len() == 2).then(|| self.s.rule_facts(&col[0].text)).flatten();
                let out = facts.and_then(|f| f.outputs.iter().find(|o| o.name == col[1].text || o.alias == col[1].text).map(|o| column_ty(f, &o.ty)));
                Some(out.unwrap_or(Ty::Unknown))
            }
        }
    }
}

/// The types of every expression, condition and cell of the file.
pub fn check(s: &Scope, out: &mut Vec<Diag>) {
    let f = s.file;
    let mut t = Typer::new(s, out);
    // the fields, and the fields of each row with the row's fields in reach
    for fd in s.fields() {
        field(&mut t, fd);
        if !fd.children.is_empty() {
            t.row = Some(&fd.children);
            for c in &fd.children {
                field(&mut t, c);
            }
            t.row = None;
        }
    }
    // the route
    match &f.route {
        Some(Route::Table(rt)) => {
            let cols: Vec<Option<Ty>> = rt.inputs.iter().map(|c| t.column(c)).collect();
            for r in &rt.rows {
                for (k, (test, _)) in r.tests.iter().enumerate() {
                    if let (Some(Some(ty)), Some(c)) = (cols.get(k), rt.inputs.get(k)) {
                        t.test(test, ty, &dotted(c));
                    }
                }
            }
        }
        Some(Route::Stages(lines)) => {
            for l in lines {
                if let Some(w) = &l.when {
                    t.cond(w);
                }
            }
        }
        _ => {}
    }
    // the tables: a column that names a field holds its values (one whose rows name ways to choose
    // an approver instead is E002's, and is read no further)
    for tb in &f.tables {
        for r in &tb.rows {
            for (k, (test, _)) in r.tests.iter().enumerate() {
                if let Some(col) = tb.inputs.get(k)
                    && let Some(fd) = s.field(&col.text)
                    && !crate::names::ways_in_field_column(s, tb, k)
                {
                    let want = t.value_ty(fd);
                    t.test(test, &want, &col.text);
                }
            }
        }
    }
    // the ways to choose an approver that read a field
    for w in &f.who {
        match &w.how {
            How::ChosenBy { field, .. } => {
                if let Some(fd) = s.field(&field.text)
                    && fd.ty != TypeExpr::Person
                {
                    let (n, ts) = (&fd.name.text, fd.ty.word());
                    t.e009(field.span, tr!("指名に使うフィールドは `person` です。`{n}` は `{ts}` です", "The field a person is chosen in is a `person`, and `{n}` is `{ts}`"));
                }
            }
            How::Lookup { path, by } => {
                let ty = t.lookup(path, by);
                if !matches!(ty, Ty::Person | Ty::Unknown) {
                    let (whole, ts) = (dotted(path), ty.shown());
                    t.e009(path[0].span, tr!("`lookup` で引く承認者は `person` のフィールドです。`{whole}` は `{ts}` です", "The approver `lookup` finds is a field of `person`, and `{whole}` is `{ts}`"));
                }
            }
            _ => {}
        }
    }
    // the ticket
    if let Some(tk) = &f.ticket {
        for l in &tk.lines {
            if let TicketLine::On { call, when, .. } = l {
                for a in &call.args {
                    t.expr(&a.value);
                }
                if let Some(w) = when {
                    t.cond(w);
                }
            }
        }
    }
    // the claims, the examples, the scenarios
    for c in &f.claims {
        if let ClaimBody::OnlyWhen { when, .. } = &c.body {
            t.cond(when);
        }
    }
    if let Some(e) = &f.examples {
        let cols: Vec<Ty> = e.inputs.iter().map(|c| t.column(c).unwrap_or(Ty::Unknown)).collect();
        for r in &e.rows {
            for (k, (v, _)) in r.values.iter().enumerate() {
                if let (Some(ty), Some(c)) = (cols.get(k), e.inputs.get(k)) {
                    let n = dotted(c);
                    t.fits(v, ty, &tr!("列 `{n}`", "the column `{n}`"));
                }
            }
        }
    }
    for sc in &f.scenarios {
        for g in &sc.given {
            for a in &g.args {
                t.expr(&a.value);
            }
        }
        for (n, v) in &sc.form {
            if let Some(fd) = s.field(&n.text) {
                t.field_value(v, fd);
            }
        }
    }
}

/// A field's computed value and its conditions.
fn field<'a>(t: &mut Typer<'_, 'a>, f: &'a Field) {
    if let Some(e) = &f.computed {
        let n = &f.name.text;
        let want = Ty::of_field(f);
        let got = t.expr(e);
        if !agree(&got, &want) {
            let (w, gs, ws) = (t.text(e), got.shown(), want.shown());
            t.e009(e.span, tr!("`{n}` は `{ws}` ですが、計算する `{w}` は `{gs}` です", "`{n}` is `{ws}`, and what computes it, `{w}`, is `{gs}`"));
        }
    }
    for c in [&f.required_when, &f.shown_when].into_iter().flatten() {
        t.cond(c);
    }
}
