//! The words of the pages (DESIGN 11): what a request's file holds, said in the page's language for
//! the people who approve and the people who manage the requests, with no line of a rule, no lowered
//! file and no output of one in them.
//!
//! A field, a value of a choice, a stage and a way to choose an approver are named by their texts in
//! the page's language: the file's titles where it is written in it, else its `labels`, else the name
//! as written. A number is written as the screens write it (ritsu's `formatSteps`: `1.25 USD`,
//! `1.25円`, `31.25%`, a whole part of five digits or more grouped by commas): the pages are read
//! beside the screens by people who fill in and approve requests, where a diagnostic writes a number
//! as the file writes it, for the person who corrects the file.

use crate::ast::{At, AtTop, CmpOp, Deadline, Expr, ExprKind, Field, File, Group, How, LabelKey, Late, Quorum, Spec, Test, TypeExpr, UseKind, WhenEmpty, Word};
use crate::lower::words::Val;
use crate::names::Scope;
use crate::org::{Org, Version};
use ritsu_base::text::Lang;
use ritsu_units::{Dim, Rat};

/// The words of one page: the file whose names it says, in the page's language, with the version of
/// the organization people and teams are named from.
pub struct Say<'s, 'a> {
    pub s: &'s Scope<'a>,
    pub lang: Lang,
    pub org: Option<&'s Org>,
    /// The version people and teams are named from: the last one the page reads.
    pub named_from: Option<&'s Version>,
}

impl<'s, 'a> Say<'s, 'a> {
    pub fn new(s: &'s Scope<'a>, lang: Lang) -> Say<'s, 'a> {
        let org = s.org_data();
        Say { s, lang, org, named_from: org.and_then(|o| o.versions.last()) }
    }

    /// The English or the Japanese, by the page's language.
    pub fn t(&self, en: impl Into<String>, ja: impl Into<String>) -> String {
        match self.lang {
            Lang::En => en.into(),
            Lang::Ja => ja.into(),
        }
    }

    pub fn ja(&self) -> bool {
        self.lang == Lang::Ja
    }

    /// A condition as a clause of when it holds: `when …`; in Japanese `…のとき` after a noun and
    /// `…とき` after a verb or an adjective, with what is said of a value that is not there after it.
    pub fn when(&self, c: String) -> String {
        match self.lang {
            Lang::En => format!("when {c}"),
            Lang::Ja => {
                const ALSO: &str = "（空のときも）";
                let (core, also) = match c.strip_suffix(ALSO) {
                    Some(x) => (x.to_string(), ALSO),
                    None => (c.clone(), ""),
                };
                if core == "いつも" {
                    return core;
                }
                let verb = core.ends_with('い') || core.ends_with('る') || core.ends_with('た');
                format!("{core}{}とき{also}", if verb { "" } else { "の" })
            }
        }
    }

    /// Whether the file's own titles are in the page's language.
    pub fn own(&self, f: &File) -> bool {
        crate::r#gen::written_in(f) == self.lang.code()
    }

    /// A label of `labels <the page's language>` the key `is` takes, in the file `f`.
    fn label_in(&self, f: &File, is: impl Fn(&LabelKey) -> bool) -> Option<String> {
        f.labels.iter().filter(|b| b.lang.text == self.lang.code()).find_map(|b| b.entries.iter().find(|e| is(&e.key)).map(|e| e.text.clone()))
    }

    /// A file's title in the page's language: its own, its label, else its name.
    pub fn title_of(&self, f: &File) -> String {
        let written = f.title.clone().unwrap_or_else(|| f.name.text.clone());
        if self.own(f) {
            return written;
        }
        self.label_in(f, |k| matches!(k, LabelKey::Request)).unwrap_or(written)
    }

    /// A file's title in the page's language, when it has one: its own, or its label.
    pub fn title_in(&self, f: &File) -> Option<String> {
        if self.own(f) {
            return f.title.clone();
        }
        self.label_in(f, |k| matches!(k, LabelKey::Request))
    }

    /// The request's title.
    pub fn title(&self) -> String {
        self.title_of(self.s.file)
    }

    /// A field, by the names of the rows it is in and its own (`["items", "price"]`), as written or
    /// in ASCII.
    pub fn field(&self, path: &[&str]) -> String {
        let f = self.s.file;
        let found = find_field(self.s.fields(), path);
        let written = found.and_then(|x| x.title.clone()).unwrap_or_else(|| path.last().map(|p| p.to_string()).unwrap_or_default());
        if self.own(f) {
            return written;
        }
        let names = written_path(self.s.fields(), path);
        self.label_in(f, |k| matches!(k, LabelKey::Field(ws) if same_path(ws, &names))).unwrap_or(written)
    }

    /// A value of a choice of the field at `path`; a value of another kind of field as it is.
    pub fn value(&self, path: &[&str], value: &str) -> String {
        let f = self.s.file;
        let field = find_field(self.s.fields(), path);
        let Some(o) = field.and_then(|x| x.options.iter().find(|o| o.name.text == value || o.name.ascii() == value)) else { return value.to_string() };
        let (name, title) = (o.name.text.clone(), o.title.clone());
        if self.own(f) {
            return title.unwrap_or(name);
        }
        let mut key = written_path(self.s.fields(), path);
        key.push(name.clone());
        self.label_in(f, |k| matches!(k, LabelKey::Field(ws) if same_path(ws, &key))).or(title).unwrap_or(name)
    }

    /// A stage, by its name as written (or its ASCII name).
    pub fn stage(&self, name: &str) -> String {
        for f in self.files() {
            if let Some(n) = crate::names::own_stages(f).into_iter().find(|n| n.text == name || n.ascii() == name) {
                let written = n.text.clone();
                if self.own(f) {
                    return written;
                }
                if let Some(l) = self.label_in(f, |k| matches!(k, LabelKey::Stage(w) if w.text == written)) {
                    return l;
                }
                // a shared route's stage may be labelled in the request
                if let Some(l) = self.label_in(self.s.file, |k| matches!(k, LabelKey::Stage(w) if w.text == written)) {
                    return l;
                }
                return written;
            }
        }
        name.to_string()
    }

    /// A way to choose an approver, by its name as written (or its ASCII name).
    pub fn way(&self, name: &str) -> String {
        for f in self.files() {
            if let Some(w) = f.who.iter().find(|w| w.name.text == name || w.name.ascii() == name) {
                let written = w.name.text.clone();
                if self.own(f) {
                    return written;
                }
                if let Some(l) = self.label_in(f, |k| matches!(k, LabelKey::Who(w) if w.text == written)) {
                    return l;
                }
                if let Some(l) = self.label_in(self.s.file, |k| matches!(k, LabelKey::Who(w) if w.text == written)) {
                    return l;
                }
                return written;
            }
        }
        name.to_string()
    }

    /// The request and the shared routes it uses.
    pub fn files(&self) -> Vec<&'s File> {
        let mut out = vec![self.s.file];
        for f in self.s.shared_routes().into_iter().flatten() {
            if !out.iter().any(|x| std::ptr::eq(*x, f)) {
                out.push(f);
            }
        }
        out
    }

    /// A person by their ID and name, as the version people are named from has them.
    pub fn person(&self, id: &str) -> String {
        match self.named_from {
            Some(v) => v.who(id).get(self.lang).to_string(),
            None => id.to_string(),
        }
    }

    /// A team by its name in the data and its ID: `Accounting (`accounting`)`, `経理部（`accounting`）`.
    pub fn team(&self, id: &str) -> String {
        match self.named_from.and_then(|v| v.team(id)) {
            Some(t) => {
                let name = if self.ja() && !t.name_ja.is_empty() { &t.name_ja } else { &t.name };
                if name.is_empty() || name == id { format!("`{id}`") } else { self.t(format!("{name} (`{id}`)"), format!("{name}（`{id}`）")) }
            }
            None => format!("`{id}`"),
        }
    }

    /// A position of the organization's file, by its title where the file is written in the page's
    /// language, else by its name.
    pub fn position(&self, name: &str) -> String {
        self.org_titled(name, |f| &f.positions)
    }

    /// A kind of team of the organization's file, the same way.
    pub fn team_kind(&self, name: &str) -> String {
        self.org_titled(name, |f| &f.team_kinds)
    }

    fn org_titled(&self, name: &str, of: impl Fn(&File) -> &Vec<crate::ast::Titled>) -> String {
        let titled = self.org_file().and_then(|f| of(f).iter().find(|t| t.name.text == name || t.name.ascii() == name).map(|t| (f, t)));
        match titled {
            Some((f, t)) if self.own(f) && t.title.is_some() => {
                let title = t.title.clone().unwrap_or_default();
                self.t(format!("{title} (`{name}`)"), format!("{title}（`{name}`）"))
            }
            _ => format!("`{name}`"),
        }
    }

    /// The organization's file the request reads, itself or through the shared route it uses.
    pub fn org_file(&self) -> Option<&'s File> {
        if self.s.file.kind == crate::ast::FileKind::Organization {
            return Some(self.s.file);
        }
        if let Some(Some(f)) = self.s.org() {
            return Some(f);
        }
        None
    }

    /// Who answers a stage, in words: one way (`manager`), several, each answering (`dept_head and
    /// accounting`, 「部長と経理」), a quorum, a circulation, a summary, a rule's answer, a shared
    /// route. A dash for no stage.
    pub fn spec(&self, spec: &Spec) -> String {
        match spec {
            Spec::Empty => "—".to_string(),
            Spec::Who(ws) => {
                let names: Vec<String> = ws.iter().map(|w| self.way(&w.text)).collect();
                self.t(and_list(&names), names.join("と"))
            }
            Spec::Quorum(q, w, _) => {
                let way = self.way(&w.text);
                match q {
                    Quorum::All => self.t(format!("all of {way}"), format!("{way}の全員")),
                    Quorum::AtLeast(n) => self.t(format!("at least {n} of {way}"), format!("{way}のうち {n} 人以上")),
                    Quorum::Percent(n) => self.t(format!("at least {n}% of {way}"), format!("{way}の {n}% 以上")),
                }
            }
            Spec::Circulate(w) => {
                let way = self.way(&w.text);
                self.t(format!("circulated to {way}"), format!("{way}に回覧"))
            }
            Spec::Note(w) => self.t(format!("a summary the flow `{}` writes", w.text), format!("flow `{}` が書く要約", w.text)),
            Spec::RuleOutput(p) => self.rule_answer(p),
            Spec::Route(w) => self.t(format!("the route `{}`", w.text), format!("経路 `{}`", w.text)),
        }
    }

    /// A rule's output a stage names (`authority.decider`): the way the rule names, with the ways it
    /// can name.
    pub fn rule_answer(&self, p: &[Word]) -> String {
        let (rule, out) = match p {
            [r, o, ..] => (r.text.as_str(), o.text.as_str()),
            [r] => (r.text.as_str(), ""),
            [] => ("", ""),
        };
        let ways = self.rule_ways(rule, out);
        if ways.is_empty() {
            return self.t(format!("the one the rule `{rule}` names in `{out}`"), format!("規則 `{rule}` が `{out}` で選ぶ人"));
        }
        self.t(format!("the one the rule `{rule}` names in `{out}` ({})", or_list(&ways)), format!("規則 `{rule}` が `{out}` で選ぶ人（{}）", ways.join("、")))
    }

    /// The ways a rule's enum output can name, by their labels.
    fn rule_ways(&self, rule: &str, out: &str) -> Vec<String> {
        let Some(facts) = self.s.rule_facts(rule) else { return Vec::new() };
        let Some(col) = facts.outputs.iter().find(|c| c.name == out || c.alias == out) else { return Vec::new() };
        let ty = match &col.ty {
            ritsu_ports::ColumnType::Opt(t) => t.as_ref(),
            t => t,
        };
        let ritsu_ports::ColumnType::Enum { name, .. } = ty else { return Vec::new() };
        let Some(e) = facts.enums.iter().find(|e| &e.name == name) else { return Vec::new() };
        e.values.iter().map(|v| self.way(&v.name)).collect()
    }

    /// How a way chooses its approvers (DESIGN 5.2), in a sentence's words.
    pub fn how(&self, h: &How) -> String {
        match h {
            How::Person(id, _) => {
                let p = self.person(id);
                self.t(format!("{p}, always this person"), format!("{p}。いつもこの人です"))
            }
            How::Team(t) => {
                let team = self.team(&t.text);
                self.t(format!("the members of the team {team}: one of them answers"), format!("{team}の人。だれか一人が答えます"))
            }
            How::ManagerOf => self.t(
                "the requester's manager: the head of the team the requester is mainly in, or of the team above where that head is the requester or there is none",
                "申請者の上長。申請者が主に属するチームの長で、その長が申請者自身か空なら、上のチームの長です",
            ),
            How::HeadOf(k) => {
                let kind = self.team_kind(&k.text);
                self.t(format!("the head of the first team of the kind {kind} above the requester"), format!("申請者から上へたどって最初の、種類が{kind}のチームの長"))
            }
            How::Nearest(p) => {
                let pos = self.position(&p.text);
                self.t(format!("the first head above the requester whose position is {pos} or higher"), format!("申請者から上へチームの長をたどって、役職が{pos}以上の最初の人"))
            }
            How::ChosenBy { previous: false, field } => {
                let f = self.field(&[field.text.as_str()]);
                self.t(format!("the person the requester enters in “{f}”"), format!("申請者が「{f}」に入れた人"))
            }
            How::ChosenBy { previous: true, field } => {
                let f = self.field(&[field.text.as_str()]);
                self.t(format!("the person the approver of the stage before enters in “{f}”"), format!("前のステップの承認者が「{f}」に入れた人"))
            }
            How::Lookup { path, by } => {
                let f = self.field(&[by.text.as_str()]);
                let col = self.master_column(path);
                self.t(format!("the person in {col} of the row chosen in “{f}”"), format!("「{f}」で選んだ行の{col}の人"))
            }
            How::Agent { flow, otherwise } => {
                let o = self.way(&otherwise.text);
                self.t(
                    format!("the agent the flow `{}` runs answers; where it is not sure, or fails, “{o}” answers in its place", flow.text),
                    format!("flow `{}` のエージェントが答えます。確かでないときと失敗したときは、代わりに「{o}」が答えます", flow.text),
                )
            }
        }
    }

    /// A number of a field of the type `ty`, as the screens write it.
    pub fn amount(&self, ty: &TypeExpr, v: Rat) -> String {
        let TypeExpr::Num(u) = ty else { return crate::lex::decimal(v) };
        let n = v.div(crate::lower::words::step(u));
        if n.is_int() { crate::screens::amount_text(ty, n.num, self.lang.code()) } else { crate::lower::words::number(v, u) }
    }

    /// A value of a field of the type `ty`: a number in its unit, or a day.
    pub fn val(&self, ty: &TypeExpr, v: Val) -> String {
        match v {
            Val::Day(d) => crate::lex::date_text(d),
            Val::Num(r) => self.amount(ty, r),
        }
    }

    /// Both ends in: `from 1 JPY to 10,000,000 JPY`, `1 JPY or more`, `10,000,000 JPY or less`.
    pub fn between(&self, lo: Option<String>, hi: Option<String>) -> String {
        match (lo, hi) {
            (Some(a), Some(b)) if a == b => a,
            (Some(a), Some(b)) => self.t(format!("from {a} to {b}"), format!("{a}から{b}まで")),
            (Some(a), None) => self.t(format!("{a} or more"), format!("{a}以上")),
            (None, Some(b)) => self.t(format!("{b} or less"), format!("{b}以下")),
            (None, None) => self.t("any value", "どの値でも"),
        }
    }

    /// The ends of an interval of a field's values, both in.
    pub fn span(&self, ty: &TypeExpr, lo: Option<Rat>, hi: Option<Rat>) -> String {
        let w = |r: Rat| match ty {
            TypeExpr::Date => crate::lex::date_text(r.num as i64),
            _ => self.amount(ty, r),
        };
        self.between(lo.map(w), hi.map(w))
    }

    /// A count of characters or rows: `1 to 200 characters`, `up to 20 rows`.
    pub fn count(&self, lo: Option<i128>, hi: Option<i128>, rows: bool) -> String {
        let (one, many, ja) = if rows { ("row", "rows", "行") } else { ("character", "characters", "字") };
        let word = |n: i128| if n == 1 { one } else { many };
        match (lo.filter(|n| *n > 0), hi) {
            (Some(a), Some(b)) if a == b => self.t(format!("{} {}", grouped(a), word(a)), format!("{}{ja}", grouped(a))),
            (Some(a), Some(b)) => self.t(format!("{} to {} {}", grouped(a), grouped(b), word(b)), format!("{}{ja}から{}{ja}まで", grouped(a), grouped(b))),
            (Some(a), None) => self.t(format!("{} {} or more", grouped(a), word(a)), format!("{}{ja}以上", grouped(a))),
            (None, Some(b)) => self.t(format!("up to {} {}", grouped(b), word(b)), format!("{}{ja}まで", grouped(b))),
            (None, None) => String::new(),
        }
    }

    /// The type of a field, in words.
    pub fn ty(&self, f: &Field, path: &[&str]) -> String {
        match &f.ty {
            TypeExpr::Text => self.t("text, one line", "一行の文"),
            TypeExpr::LongText => self.t("text", "文"),
            TypeExpr::Num(u) => match &u.dim {
                Dim::Number => self.t("number", "数"),
                Dim::Rate => {
                    let st = crate::lex::decimal(crate::lower::words::step(u).mul(Rat::int(100)));
                    self.t(format!("rate, in steps of {st}%"), format!("率（{st}% 刻み）"))
                }
                Dim::Money(_) => {
                    let c = crate::screens::currency(&f.ty).unwrap_or_else(|| u.unit.clone());
                    let c = if c == "JPY" && self.ja() { "円".to_string() } else { c };
                    self.t(format!("amount in {c}"), format!("金額（{c}）"))
                }
                _ => self.t(format!("number in {}", u.unit), format!("数（{}）", u.unit)),
            },
            TypeExpr::Date => match &f.open_in {
                Some(c) => self.t(format!("date, a business day of the calendar `{}`", c.text), format!("日付（カレンダー `{}` の営業日）", c.text)),
                None => self.t("date", "日付"),
            },
            TypeExpr::Choice => {
                let vs: Vec<String> = f.options.iter().map(|o| self.value(path, &o.name.text)).collect();
                self.t(format!("one of {}", or_list(&vs)), format!("{}のどれか", vs.join("、")))
            }
            TypeExpr::ChoiceFrom(p) => {
                let dotted = crate::ast::dotted(p);
                if let Some(t) = self.master_title(&dotted) {
                    return self.t(format!("a row of {t}"), format!("{t}の行"));
                }
                match self.rule_values(p) {
                    Some(vs) => self.t(format!("one of {}", or_list(&vs)), format!("{}のどれか", vs.join("、"))),
                    None => self.t(format!("one of the values of `{dotted}`"), format!("`{dotted}` の値のどれか")),
                }
            }
            TypeExpr::Attachment => self.t("a file", "添付ファイル"),
            TypeExpr::Person => self.t("a person of the organization", "組織の人"),
            TypeExpr::Team => self.t("a team of the organization", "組織のチーム"),
            TypeExpr::Bool => self.t("yes or no", "はい・いいえ"),
            TypeExpr::Rows => self.t("rows", "明細の行"),
        }
    }

    /// The values of a rule's enum a field chooses from (`authority.kind`): the enum of that name, or
    /// of the rule's column of that name; each as the rule names it in public.
    fn rule_values(&self, p: &[Word]) -> Option<Vec<String>> {
        let [r, e] = p else { return None };
        let facts = self.s.rule_facts(&r.text)?;
        let by_name = facts.enums.iter().find(|x| x.name == e.text || x.public == e.text || x.alias == e.text);
        let of_column = || {
            let c = facts.inputs.iter().chain(&facts.outputs).find(|c| c.name == e.text || c.alias == e.text)?;
            let ty = match &c.ty {
                ritsu_ports::ColumnType::Opt(t) => t.as_ref(),
                t => t,
            };
            let ritsu_ports::ColumnType::Enum { name, .. } = ty else { return None };
            facts.enums.iter().find(|x| &x.name == name)
        };
        let en = by_name.or_else(of_column)?;
        Some(en.values.iter().map(|v| format!("`{}`", v.public)).collect())
    }

    /// The title of a master the request reads (`m.accounts`), with its name, when it reads one by
    /// that name: ``Account codes (`m.accounts`)``; its name alone in a page of another language than
    /// the masters' file.
    pub fn master_title(&self, p: &str) -> Option<String> {
        let (alias, master) = p.split_once('.')?;
        let mf = self.s.ringi(alias, UseKind::Masters)?;
        let m = mf.masters.iter().find(|m| m.name.text == master || m.name.ascii() == master)?;
        Some(match (&m.title, self.own(mf)) {
            (Some(t), true) => self.t(format!("{t} (`{p}`)"), format!("{t}（`{p}`）")),
            _ => format!("`{p}`"),
        })
    }

    /// A field of a master a lookup reads (`m.systems.owner`): its title where the masters are
    /// written in the page's language, else its name.
    fn master_column(&self, path: &[Word]) -> String {
        let dotted = format!("`{}`", crate::ast::dotted(path));
        let [alias, master, field] = path else { return dotted };
        let Some(mf) = self.s.ringi(&alias.text, UseKind::Masters) else { return dotted };
        let Some(m) = mf.masters.iter().find(|m| m.name.text == master.text || m.name.ascii() == master.text) else { return dotted };
        let Some(x) = m.fields.iter().find(|x| x.name.text == field.text || x.name.ascii() == field.text) else { return dotted };
        match (&x.title, self.own(mf)) {
            (Some(t), true) => self.t(format!("“{t}”"), format!("「{t}」")),
            _ => format!("`{}`", x.name.text),
        }
    }

    /// A condition in words (a field's `shown when`, `required when`, a stage's `when`, an `on`
    /// line's `when`): `is none` and `is not none` said plainly, and `is not` saying that it holds
    /// of a value that is not there too (DESIGN 4.9). `row` is the `rows` field whose row it is read
    /// in. What these words do not follow is written as the file writes it.
    pub fn cond(&self, e: &Expr, row: Option<&Field>) -> String {
        match &e.kind {
            ExprKind::And(a, b) => self.t(format!("{} and {}", self.cond(a, row), self.cond(b, row)), format!("{}、かつ{}", self.cond(a, row), self.cond(b, row))),
            ExprKind::Or(a, b) => self.t(format!("{}, or {}", self.cond(a, row), self.cond(b, row)), format!("{}か、{}", self.cond(a, row), self.cond(b, row))),
            ExprKind::Not(a) => self.t(format!("it is not so that {}", self.cond(a, row)), format!("「{}」でない", self.cond(a, row))),
            ExprKind::Cmp(op, a, b) => self.compare(*op, a, b, row),
            ExprKind::Path(p) => self.holds(p, row),
            ExprKind::Bool(true) => self.t("always", "いつも"),
            ExprKind::Bool(false) => self.t("never", "どのときも成り立たない"),
            _ => format!("`{}`", self.written(e)),
        }
    }

    /// A bool a path names holds: a check box is ticked, a rule answers yes.
    fn holds(&self, p: &[Word], row: Option<&Field>) -> String {
        let names: Vec<&str> = p.iter().map(|w| w.text.as_str()).collect();
        if let [r, o] = names.as_slice()
            && self.s.field(r).is_none()
        {
            return self.t(format!("the rule `{r}` answers yes in `{o}`"), format!("規則 `{r}` が `{o}` に「はい」と答える"));
        }
        let f = self.label_of(&names, row);
        self.t(format!("“{f}” is yes"), format!("「{f}」が「はい」"))
    }

    /// The label of a field a condition names: in the row it is read in, when it is one of its.
    fn label_of(&self, names: &[&str], row: Option<&Field>) -> String {
        match row {
            Some(r) if names.len() == 1 && r.children.iter().any(|c| c.name.text == names[0]) => self.field(&[r.name.text.as_str(), names[0]]),
            _ => self.field(names),
        }
    }

    /// The field a path names, when it names one of the row or of the form, with the path a value
    /// of it is labelled under.
    fn field_of(&self, p: &[Word], row: Option<&Field>) -> Option<(&'s Field, Vec<String>)> {
        let names: Vec<&str> = p.iter().map(|w| w.text.as_str()).collect();
        if let (Some(r), [one]) = (row, names.as_slice())
            && let Some(rf) = find_field(self.s.fields(), &[r.name.text.as_str()])
            && let Some(f) = rf.children.iter().find(|x| x.name.text == *one)
        {
            return Some((f, vec![rf.name.text.clone(), f.name.text.clone()]));
        }
        find_field(self.s.fields(), &names).map(|f| (f, names.iter().map(|n| n.to_string()).collect()))
    }

    /// One side of a comparison: a field by its label, the requester's team of a kind, a rule's
    /// answer, a value.
    fn term(&self, e: &Expr, ty: Option<&TypeExpr>, row: Option<&Field>) -> String {
        match &e.kind {
            ExprKind::Path(p) => {
                let names: Vec<&str> = p.iter().map(|w| w.text.as_str()).collect();
                match names.as_slice() {
                    ["requester", kind] => {
                        let k = self.team_kind(kind);
                        self.t(format!("the requester's team of the kind {k}"), format!("申請者の上の、種類が{k}のチーム"))
                    }
                    ["requester"] => self.t("the requester", "申請者"),
                    [r, o] if self.s.field(r).is_none() => self.t(format!("what the rule `{r}` answers in `{o}`"), format!("規則 `{r}` が `{o}` に答える値")),
                    _ => {
                        let f = self.label_of(&names, row);
                        self.t(format!("“{f}”"), format!("「{f}」"))
                    }
                }
            }
            ExprKind::Sum(p) => {
                let names: Vec<&str> = p.iter().map(|w| w.text.as_str()).collect();
                let (rows, col) = (self.field(&names[..1]), self.field(&names));
                self.t(format!("the total of “{col}” over the rows of “{rows}”"), format!("「{rows}」の行の「{col}」の合計"))
            }
            ExprKind::Num(_) | ExprKind::Neg(_) | ExprKind::Date(_) => match ty {
                Some(t) => match crate::lower::words::constant_in(e, t) {
                    Some(v) => self.val(t, v),
                    None => format!("`{}`", self.written(e)),
                },
                None => match crate::lower::words::constant(e) {
                    Some(Val::Day(d)) => crate::lex::date_text(d),
                    _ => format!("`{}`", self.written(e)),
                },
            },
            ExprKind::Str(s) => format!("\"{s}\""),
            ExprKind::Bool(b) => self.t(if *b { "yes" } else { "no" }, if *b { "「はい」" } else { "「いいえ」" }),
            ExprKind::Person(id) => self.person(id),
            ExprKind::Team(t) => self.team(&t.text),
            _ => format!("`{}`", self.written(e)),
        }
    }

    fn compare(&self, op: CmpOp, a: &Expr, b: &Expr, row: Option<&Field>) -> String {
        let found = match &a.kind {
            ExprKind::Path(p) => self.field_of(p, row),
            _ => None,
        };
        let field = found.as_ref().map(|(f, _)| *f);
        let ty = field.map(|f| &f.ty);
        let path: Vec<&str> = match &a.kind {
            ExprKind::Path(p) => p.iter().map(|w| w.text.as_str()).collect(),
            _ => Vec::new(),
        };
        let x = self.term(a, None, row);
        // a value that may not be there: `is not` holds of it too (DESIGN 4.9)
        let absent = field.is_some_and(|f| crate::lower::may_be_empty(f) || (f.computed.is_some() && crate::lower::may_be_none(self.s, f)));
        match (op, &b.kind) {
            (CmpOp::Is, ExprKind::None) => match path.as_slice() {
                ["requester", kind] => {
                    let k = self.team_kind(kind);
                    self.t(format!("no team of the kind {k} is above the requester"), format!("申請者の上に種類が{k}のチームが無い"))
                }
                _ => self.t(format!("{x} has no value"), format!("{x}が空")),
            },
            (CmpOp::IsNot, ExprKind::None) => match path.as_slice() {
                ["requester", kind] => {
                    let k = self.team_kind(kind);
                    self.t(format!("a team of the kind {k} is above the requester"), format!("申請者の上に種類が{k}のチームがある"))
                }
                _ => self.t(format!("{x} has a value"), format!("{x}が空でない")),
            },
            (CmpOp::Is | CmpOp::IsNot, _) => {
                let v = match (&b.kind, &found) {
                    (ExprKind::Path(p), Some((f, at))) if p.len() == 1 && matches!(f.ty, TypeExpr::Choice) => {
                        let at: Vec<&str> = at.iter().map(String::as_str).collect();
                        let v = self.value(&at, &p[0].text);
                        self.t(format!("“{v}”"), format!("「{v}」"))
                    }
                    (ExprKind::Path(p), Some((f, _))) if p.len() == 1 && matches!(f.ty, TypeExpr::ChoiceFrom(_)) => format!("`{}`", p[0].text),
                    _ => self.term(b, ty, row),
                };
                if op == CmpOp::Is {
                    self.t(format!("{x} is {v}"), format!("{x}が{v}"))
                } else if absent {
                    self.t(format!("{x} is not {v} (or has no value)"), format!("{x}が{v}でない（空のときも）"))
                } else {
                    self.t(format!("{x} is not {v}"), format!("{x}が{v}でない"))
                }
            }
            (op, _) => {
                let v = self.term(b, ty, row);
                match op {
                    CmpOp::Gt => self.t(format!("{x} is more than {v}"), format!("{x}が{v}より大きい")),
                    CmpOp::Ge => self.t(format!("{x} is {v} or more"), format!("{x}が{v}以上")),
                    CmpOp::Lt => self.t(format!("{x} is less than {v}"), format!("{x}が{v}未満")),
                    CmpOp::Le => self.t(format!("{x} is {v} or less"), format!("{x}が{v}以下")),
                    CmpOp::Is | CmpOp::IsNot => unreachable!(),
                }
            }
        }
    }

    /// An expression as the file writes it.
    pub fn written(&self, e: &Expr) -> String {
        crate::types::written(self.s.src, e.span)
    }

    /// A computation in words: a total, a lookup, the fields it works with by their labels and the
    /// signs of arithmetic (`“Quantity” × “Unit price”`); else as written. `row` is the `rows` field
    /// whose row it is worked out in.
    pub fn computed(&self, e: &Expr, row: Option<&Field>) -> String {
        match &e.kind {
            ExprKind::Lookup(p, by) => {
                let f = self.field(&[by.text.as_str()]);
                let col = self.master_column(p);
                self.t(format!("{col} of the row chosen in “{f}”"), format!("「{f}」で選んだ行の{col}"))
            }
            _ => self.arith(e, row).unwrap_or_else(|| format!("`{}`", self.written(e))),
        }
    }

    /// An arithmetic expression in words, with brackets where the order needs them; None for what
    /// it does not follow.
    fn arith(&self, e: &Expr, row: Option<&Field>) -> Option<String> {
        use crate::ast::BinOp;
        let rank = |op: BinOp| if matches!(op, BinOp::Add | BinOp::Sub) { 1 } else { 2 };
        match &e.kind {
            // a day and business days: `N business days after “First day”`
            ExprKind::Bin(op @ (BinOp::Add | BinOp::Sub), a, b) if matches!(b.kind, ExprKind::BusinessDays(..)) => {
                let ExprKind::BusinessDays(n, cal) = &b.kind else { return None };
                let (from, n) = (self.arith(a, row)?, self.arith(n, row)?);
                let after = *op == BinOp::Add;
                Some(match cal {
                    Some(c) => self.t(
                        format!("{n} business days of the calendar `{}` {} {from}", c.text, if after { "after" } else { "before" }),
                        format!("{from}から、カレンダー `{}` の営業日で{n}日{}", c.text, if after { "あと" } else { "前" }),
                    ),
                    None => self.t(format!("{n} business days {} {from}", if after { "after" } else { "before" }), format!("{from}から、営業日で{n}日{}", if after { "あと" } else { "前" })),
                })
            }
            ExprKind::Bin(op, a, b) => {
                let side = |x: &Expr, right: bool| -> Option<String> {
                    let s = self.arith(x, row)?;
                    let bracket = match &x.kind {
                        ExprKind::Bin(inner, ..) => rank(*inner) < rank(*op) || (right && rank(*inner) == rank(*op) && matches!(op, BinOp::Sub | BinOp::Div)),
                        _ => false,
                    };
                    Some(if bracket { format!("({s})") } else { s })
                };
                let sign = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "−",
                    BinOp::Mul => "×",
                    BinOp::Div => "÷",
                };
                Some(format!("{} {sign} {}", side(a, false)?, side(b, true)?))
            }
            ExprKind::Neg(a) => Some(format!("−{}", self.arith(a, row)?)),
            ExprKind::Path(_) | ExprKind::Sum(_) => Some(self.term(e, None, row)),
            ExprKind::Num(n) => {
                let v = if n.value.is_int() { grouped(n.value.num) } else { crate::lex::decimal(n.value) };
                Some(match n.unit.as_str() {
                    "" => v,
                    "%" => format!("{v}%"),
                    "JPY" | "円" if self.ja() => format!("{v}円"),
                    u if self.ja() => format!("{v}{u}"),
                    u => format!("{v} {u}"),
                })
            }
            ExprKind::BusinessDays(n, cal) => {
                let n = self.arith(n, row)?;
                Some(match cal {
                    Some(c) => self.t(format!("{n} business days of the calendar `{}`", c.text), format!("カレンダー `{}` で {n} 営業日", c.text)),
                    None => self.t(format!("{n} business days"), format!("{n} 営業日")),
                })
            }
            _ => None,
        }
    }

    /// A cell of the route's table (or of its examples) in words, under the column that reads `col`.
    pub fn test(&self, col: &[Word], t: &Test) -> String {
        let names: Vec<&str> = col.iter().map(|w| w.text.as_str()).collect();
        let field = find_field(self.s.fields(), &names);
        let ty = field.map(|f| &f.ty);
        let one = |e: &Expr| -> String {
            match (&e.kind, field) {
                (ExprKind::None, _) => self.t("no value", "空"),
                (ExprKind::Bool(b), _) => self.t(if *b { "yes" } else { "no" }, if *b { "はい" } else { "いいえ" }),
                (ExprKind::Path(p), Some(f)) if p.len() == 1 && matches!(f.ty, TypeExpr::Choice) => self.value(&names, &p[0].text),
                (ExprKind::Path(p), _) if p.len() == 1 => format!("`{}`", p[0].text),
                _ => self.term(e, ty, None),
            }
        };
        match t {
            Test::Any => self.t("any", "問わない"),
            Test::Values { values, not: false } => {
                let vs: Vec<String> = values.iter().map(one).collect();
                self.t(or_list(&vs), vs.join("か"))
            }
            Test::Values { values, not: true } => {
                let vs: Vec<String> = values.iter().map(one).collect();
                self.t(format!("anything but {} (or no value)", or_list(&vs)), format!("{}以外（空も）", vs.join("と")))
            }
            Test::Compare(cs) => {
                let two = cs.len() > 1;
                let parts: Vec<String> = cs
                    .iter()
                    .enumerate()
                    .map(|(i, (op, e))| {
                        let v = self.term(e, ty, None);
                        let first = two && i + 1 < cs.len();
                        match op {
                            CmpOp::Gt if first => self.t(format!("more than {v}"), format!("{v}より大きく")),
                            CmpOp::Gt => self.t(format!("more than {v}"), format!("{v}より大きい")),
                            CmpOp::Ge if first => self.t(format!("{v} or more"), format!("{v}以上で")),
                            CmpOp::Ge => self.t(format!("{v} or more"), format!("{v}以上")),
                            CmpOp::Lt => self.t(format!("less than {v}"), format!("{v}未満")),
                            CmpOp::Le => self.t(format!("{v} or less"), format!("{v}以下")),
                            CmpOp::Is => v,
                            CmpOp::IsNot => self.t(format!("not {v}"), format!("{v}でない")),
                        }
                    })
                    .collect();
                self.t(parts.join(" and "), parts.join("、"))
            }
        }
    }

    /// The header of a column of the route's table: a field's label, or a rule's output.
    pub fn column(&self, col: &[Word]) -> String {
        let names: Vec<&str> = col.iter().map(|w| w.text.as_str()).collect();
        match names.as_slice() {
            [r, o] if self.s.field(r).is_none() => self.t(format!("The rule `{r}`: `{o}`"), format!("規則 `{r}` の `{o}`")),
            _ => self.field(&names),
        }
    }

    /// A time of the day a deadline is at, after the day: `at the end of the day`, `at 17:00`.
    pub fn at(&self, at: At) -> String {
        match at {
            At::EndOfDay => self.t("at the end of the day", "の終わり"),
            At::Time(t) => {
                let t = crate::lex::time_text(t);
                self.t(format!("at {t}"), format!("の {t}"))
            }
        }
    }

    /// `3 business days` (of the calendar `office` when one is named).
    pub fn days(&self, n: u32, cal: Option<&Word>) -> String {
        let unit = self.t(if n == 1 { "business day" } else { "business days" }, "営業日");
        match cal {
            Some(c) => self.t(format!("{n} {unit} of the calendar `{}`", c.text), format!("カレンダー `{}` で {n} {unit}", c.text)),
            None => format!("{n} {unit}"),
        }
    }

    /// A line of `every stage` or under a stage, in words.
    pub fn deadline(&self, d: &Deadline) -> String {
        match d {
            Deadline::Due { days, calendar, at, .. } => {
                let (n, a) = (self.days(*days, calendar.as_ref()), self.at(*at));
                self.t(format!("It is due {n} after it starts, {a}."), format!("期限は、始まった日から {n}の日{a}です。"))
            }
            Deadline::Remind { days, calendar, at, .. } => {
                let (n, t) = (self.days(*days, calendar.as_ref()), crate::lex::time_text(*at));
                self.t(format!("Its approvers are reminded {n} after it starts, at {t}."), format!("始まった日から {n}の日の {t} に、承認者へ催促が届きます。"))
            }
            Deadline::Late { first, then, .. } => {
                let one = |l: &Late| match l {
                    Late::Escalate(Some(t)) => self.t(format!("it goes up as the table `{}` says", t.text), format!("表 `{}` のとおりに上へ回ります", t.text)),
                    Late::Escalate(None) => self.t("it goes up to the head above", "上の長へ回ります"),
                    Late::Expire => self.t("the ticket expires", "チケットは期限切れで終わります"),
                    Late::Stay => self.t("it waits on", "そのまま待ちます"),
                };
                match then {
                    Some(t) => self.t(
                        format!("When it is late, {}, and its deadline is counted again from then; when it is late again, {}.", one(first), one(t)),
                        format!("期限を過ぎると{}。期限は、そこから数え直されます。また過ぎると{}。", one(first), one(t)),
                    ),
                    None => self.t(format!("When it is late, {}.", one(first)), format!("期限を過ぎると{}。", one(first))),
                }
            }
            Deadline::Empty { what, .. } => self.empty(what),
        }
    }

    /// What a stage does when nobody is there to answer it (DESIGN 5.4).
    pub fn empty(&self, w: &WhenEmpty) -> String {
        match w {
            WhenEmpty::Stay => self.t("When nobody can answer it, it waits for an administrator to add an approver.", "答える人がいなければ、管理する人が承認者を足すまで待ちます。"),
            WhenEmpty::Skip => self.t("When nobody can answer it, the stage is skipped.", "答える人がいなければ、ステップは飛ばされます。"),
            WhenEmpty::Escalate(top) => {
                let then = match top {
                    None | Some(AtTop::Stay) => self.t("at the top of the organization, it waits for an administrator", "組織のいちばん上では、管理する人を待ちます"),
                    Some(AtTop::Skip) => self.t("at the top of the organization, the stage is skipped", "組織のいちばん上では、ステップは飛ばされます"),
                    Some(AtTop::To(w)) => {
                        let way = self.way(&w.text);
                        self.t(format!("at the top of the organization, it goes to “{way}”"), format!("組織のいちばん上では、「{way}」へ回ります"))
                    }
                };
                self.t(format!("When nobody can answer it, it goes up to the head above; {then}."), format!("答える人がいなければ、上の長へ回ります。{then}。"))
            }
        }
    }

    /// A group of a `submitted by` line in words.
    pub fn group(&self, g: &Group) -> String {
        let within = |w: &Option<Word>| w.as_ref().map(|t| self.team(&t.text));
        match g {
            Group::Everyone { within: w, .. } => match within(w) {
                None => self.t("everyone", "全員"),
                Some(t) => self.t(format!("everyone in {t} and the teams below it"), format!("{t}とその下のチームの全員")),
            },
            Group::Team(t) => {
                let t = self.team(&t.text);
                self.t(format!("the members of the team {t}"), format!("{t}の人"))
            }
            Group::Position { position, higher, within: w } => {
                let p = self.position(&position.text);
                let base = if *higher { self.t(format!("the people whose position is {p} or higher"), format!("役職が{p}以上の人")) } else { self.t(format!("the people whose position is {p}"), format!("役職が{p}の人")) };
                match within(w) {
                    None => base,
                    Some(t) => self.t(format!("{base}, in {t} and the teams below it"), format!("{t}とその下のチームの、{base}")),
                }
            }
            Group::Person(id, _) => self.person(id),
        }
    }
}

/// A day with its day of the week: `2026-04-03 (Fri)`, `2026-04-03（金）`.
pub fn day_text(day: i64, lang: Lang) -> String {
    let d = crate::lex::date_text(day);
    let w = (day + 4).rem_euclid(7) as usize;
    match lang {
        Lang::En => format!("{d} ({})", ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][w]),
        Lang::Ja => format!("{d}（{}）", ["日", "月", "火", "水", "木", "金", "土"][w]),
    }
}

/// A whole number with commas from five digits: `20`, `2000`, `20,000`.
pub fn grouped(n: i128) -> String {
    crate::lower::words::grouped(n).replace('_', ",")
}

/// `a`, `a and b`, `a, b and c`.
pub fn and_list(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// `a`, `a or b`, `a, b or c`.
pub fn or_list(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// A field of the form, by the names of the rows it is in and its own (as written or ASCII).
pub fn find_field<'f>(form: &'f [Field], path: &[&str]) -> Option<&'f Field> {
    let (first, rest) = path.split_first()?;
    let f = form.iter().find(|x| x.name.text == *first || x.name.ascii() == *first)?;
    if rest.is_empty() { Some(f) } else { find_field(&f.children, rest) }
}

/// The names as written of the fields on a path given as written or in ASCII.
fn written_path(form: &[Field], path: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = form;
    for p in path {
        match at.iter().find(|x| x.name.text == *p || x.name.ascii() == *p) {
            Some(f) => {
                out.push(f.name.text.clone());
                at = &f.children;
            }
            None => out.push(p.to_string()),
        }
    }
    out
}

fn same_path(ws: &[Word], path: &[String]) -> bool {
    ws.len() == path.len() && ws.iter().zip(path).all(|(w, p)| w.text == *p)
}
