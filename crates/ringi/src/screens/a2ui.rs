//! The six screens of a kind of request in A2UI 1.0 messages (DESIGN 13.1): the form, the card,
//! the lists, the history and the notice of when a request may be made from, each in English and
//! in Japanese (13.4). A screen is a template: three
//! messages that make the surface (`createSurface`), give it the shape of its data with nothing in
//! it (`updateDataModel`), and give it its parts (`updateComponents`). The server puts the values
//! of a ticket into the data and sends them as they are; what each place of the data holds is in
//! DESIGN 13.8. The surface defaults to the basic catalog; a part or a function of ritsu's names
//! the catalog of ritsu's (`@catalogId`, `catalogId`). Every value a button sends is one the
//! screen shows, from the same place of the data (13.3): a choice is sent as its code and shown
//! through `lookup` from the same place, and a hidden field is not sent.

use super::{LANGS, SCREENS, amount_text, basic_format, currency, decimals, label, step, unit_after};
use crate::ast::TypeExpr;
use crate::r#gen::{Fld, Kind, Named};
use ritsu_base::json::Json;

/// Each screen of `k` with its language, the parts of ritsu's named by the catalog `catalog`.
pub fn screens(k: &Kind, catalog: &str) -> Vec<(&'static str, &'static str, Json)> {
    let mut out = Vec::new();
    for screen in SCREENS {
        for lang in LANGS {
            let mut s = S { k, r: catalog, lang, comps: Vec::new() };
            let data = match screen {
                "form" => s.form(),
                "card" => s.card(),
                "list" => s.list(),
                "mine" => s.mine(),
                "history" => s.history(),
                _ => s.offered(),
            };
            out.push((screen, lang, s.messages(screen, data)));
        }
    }
    out
}

/// The events a card offers, in the order it shows them: every event a person makes on a ticket
/// but making the request and making it again, which the form offers, and editing it, which the
/// card offers beside the fields an approver gives.
const CARD_EVENTS: [&str; 10] = ["approve", "reject", "send_back", "retract", "pause", "resume", "withdraw", "discard", "add_approver", "hand_over"];

/// A screen as it is made.
struct S<'a> {
    k: &'a Kind,
    /// The ID of ritsu's catalog.
    r: &'a str,
    lang: &'static str,
    comps: Vec<Json>,
}

fn path(p: &str) -> Json {
    Json::obj([("@path", Json::str(p))])
}

fn obj(pairs: Vec<(&str, Json)>) -> Json {
    Json::obj(pairs)
}

/// A call of a function of the basic catalog.
fn call(name: &str, args: Vec<(&str, Json)>) -> Json {
    obj(vec![("@call", Json::str(name)), ("args", obj(args))])
}

/// A rule of `@checks`.
fn rule(condition: Json, message: String) -> Json {
    obj(vec![("condition", condition), ("message", Json::str(message))])
}

/// A number with its thousands grouped (`1,000,000`).
fn grouped(n: i128) -> String {
    crate::lower::words::grouped(n).replace('_', ",")
}

impl<'a> S<'a> {
    fn ja(&self) -> bool {
        self.lang == "ja"
    }

    fn t(&self, en: &str, ja: &str) -> String {
        if self.ja() { ja.to_string() } else { en.to_string() }
    }

    fn name(&self) -> &str {
        &self.k.named.name
    }

    fn label(&self, n: &Named) -> String {
        label(n, self.lang)
    }

    /// A call of a function of ritsu's catalog.
    fn rcall(&self, name: &str, args: Vec<(&str, Json)>) -> Json {
        obj(vec![("@call", Json::str(name)), ("catalogId", Json::str(self.r)), ("args", obj(args))])
    }

    /// A call of one of the kind's own functions (`expense.check`).
    fn kcall(&self, f: &str, args: Vec<(&str, Json)>) -> Json {
        let name = format!("{}.{f}", self.name());
        self.rcall(&name, args)
    }

    /// Add a part of the basic catalog; its ID back.
    fn add(&mut self, id: &str, component: &str, props: Vec<(&str, Json)>) -> String {
        let mut all = vec![("id", Json::str(id)), ("component", Json::str(component))];
        all.extend(props);
        self.comps.push(obj(all));
        id.to_string()
    }

    /// Whether the screen has a part that finds its values at the server (`PersonPicker`,
    /// `MasterPicker`): its data then holds the address they find at (`/find`, DESIGN 13.8).
    fn finds(&self) -> bool {
        self.comps.iter().any(|c| matches!(c.get("component").and_then(Json::as_str), Some("PersonPicker" | "MasterPicker")))
    }

    /// Add a part of ritsu's catalog.
    fn radd(&mut self, id: &str, component: &str, props: Vec<(&str, Json)>) -> String {
        let mut all = vec![("id", Json::str(id)), ("component", Json::str(component)), ("@catalogId", Json::str(self.r))];
        all.extend(props);
        self.comps.push(obj(all));
        id.to_string()
    }

    fn text(&mut self, id: &str, text: Json) -> String {
        self.add(id, "Text", vec![("text", text)])
    }

    fn caption(&mut self, id: &str, text: &str) -> String {
        self.add(id, "Text", vec![("text", Json::str(text)), ("variant", Json::str("caption"))])
    }

    fn row(&mut self, id: &str, children: Vec<String>) -> String {
        self.add(id, "Row", vec![("children", Json::arr(children.into_iter().map(Json::str)))])
    }

    fn column(&mut self, id: &str, children: Vec<String>) -> String {
        self.add(id, "Column", vec![("children", Json::arr(children.into_iter().map(Json::str)))])
    }

    /// A part that shows `child` only while `when` holds.
    fn show(&mut self, id: &str, when: Json, child: String) -> String {
        self.radd(id, "Show", vec![("when", when), ("child", Json::str(child))])
    }

    /// The three messages of the screen: its surface, its data with nothing in it, its parts.
    fn messages(self, screen: &str, data: Json) -> Json {
        let surface = format!("{}.{screen}", self.name());
        let id = || Json::str(&surface);
        let one = |key: &str, body: Vec<(&str, Json)>| obj(vec![("version", Json::str("1.0")), (key, obj(body))]);
        Json::obj([
            ("//", Json::str(crate::r#gen::head())),
            (
                "messages",
                Json::arr([
                    one("createSurface", vec![("surfaceId", id()), ("catalogId", Json::str(super::BASIC))]),
                    one("updateDataModel", vec![("surfaceId", id()), ("value", data)]),
                    one("updateComponents", vec![("surfaceId", id()), ("components", Json::arr(self.comps))]),
                ]),
            ),
        ])
    }

    // ── the words of the screens ──

    fn event_word(&self, e: &str) -> String {
        let (en, ja) = match e {
            "submit" => ("Submit", "申請する"),
            "resubmit" => ("Submit again", "もう一度申請する"),
            "approve" => ("Approve", "承認する"),
            "reject" => ("Reject", "却下する"),
            "send_back" => ("Send back", "差し戻す"),
            "retract" => ("Take back my approval", "承認を取り消す"),
            "pause" => ("Put on hold", "保留にする"),
            "resume" => ("Take off hold", "保留を解く"),
            "withdraw" => ("Withdraw", "取り下げる"),
            "discard" => ("Discard the request", "申請をやめる"),
            "edit" => ("Save the changes", "変更を保存する"),
            "add_approver" => ("Add the approver", "承認者を加える"),
            "hand_over" => ("Hand over", "引き継ぐ"),
            "comment" => ("Post the comment", "コメントを送る"),
            _ => ("Open", "開く"),
        };
        self.t(en, ja)
    }

    /// The words of the states of a ticket, by their names in the contract.
    fn statuses(&self) -> Json {
        let all = [
            ("in_progress", "In progress", "進行中"),
            ("paused", "On hold", "保留中"),
            ("with_requester", "With the requester", "申請者に戻っています"),
            ("approved", "Approved", "承認されました"),
            ("rejected", "Rejected", "却下されました"),
            ("withdrawn", "Withdrawn", "取り下げられました"),
            ("expired", "Expired", "期限が切れました"),
        ];
        Json::obj(all.iter().map(|(k, en, ja)| (*k, Json::str(self.t(en, ja)))))
    }

    /// The words of the lines of a history, by what the line says was done.
    fn dids(&self) -> Json {
        let all = [
            ("submitted", "Submitted", "申請しました"),
            ("resubmitted", "Submitted again", "もう一度申請しました"),
            ("approved", "Approved", "承認しました"),
            ("rejected", "Rejected", "却下しました"),
            ("sent back", "Sent back", "差し戻しました"),
            ("retracted", "Took back the approval", "承認を取り消しました"),
            ("paused", "Put on hold", "保留にしました"),
            ("resumed", "Took off hold", "保留を解きました"),
            ("withdrawn", "Withdrew", "取り下げました"),
            ("edited", "Edited", "修正しました"),
            ("approver added", "Added an approver", "承認者を加えました"),
            ("handed over", "Handed over", "引き継ぎました"),
            ("commented", "Commented", "コメントしました"),
            ("reminded", "Reminded", "催促しました"),
            ("late", "Past due", "期限を過ぎました"),
            ("went up", "Went up", "上の人に回りました"),
            ("went to", "Went to", "回りました"),
            ("skipped", "Skipped", "飛ばしました"),
            ("stopped", "Stopped", "止まりました"),
            ("circulated", "Circulated", "回覧しました"),
            ("deferred", "Deferred to a later stage", "あとのステップで答えます"),
            ("deferral ended", "Answers here", "このステップで答えます"),
            ("agent asked", "Asked the agent", "エージェントに尋ねました"),
            ("agent ok", "The agent agreed", "エージェントが認めました"),
            ("agent doubt", "The agent had doubts", "エージェントが疑問を出しました"),
            ("agent failed", "The agent could not answer", "エージェントが答えられませんでした"),
            ("note asked", "Asked for a note", "メモを頼みました"),
            ("note attached", "A note was attached", "メモが付きました"),
            ("note missing", "No note came", "メモが届きませんでした"),
            ("ticket approved", "The request was approved", "申請が承認されました"),
            ("ticket rejected", "The request was rejected", "申請が却下されました"),
            ("ticket withdrawn", "The request was withdrawn", "申請が取り下げられました"),
            ("ticket expired", "The request expired", "申請の期限が切れました"),
        ];
        Json::obj(all.iter().map(|(k, en, ja)| (*k, Json::str(self.t(en, ja)))))
    }

    fn stage_table(&self) -> Json {
        Json::obj(self.k.stages.iter().map(|s| (s.name.as_str(), Json::str(self.label(s)))))
    }

    fn way_table(&self) -> Json {
        Json::obj(self.k.ways.iter().map(|w| (w.name.as_str(), Json::str(self.label(w)))))
    }

    /// The labels of the fields by their ASCII names, a row's fields as `<rows>.<field>`.
    fn field_table(&self) -> Json {
        let mut all = Vec::new();
        for f in &self.k.form {
            all.push((f.named.name.clone(), Json::str(self.label(&f.named))));
            for c in &f.fields {
                all.push((format!("{}.{}", f.named.name, c.named.name), Json::str(self.label(&c.named))));
            }
        }
        Json::obj(all)
    }

    // ── the checks, in the words of the screen ──

    /// A value of `f` as a screen says it: a number in its unit as `formatSteps` writes it
    /// (`amount_text`), a day as it is.
    fn value_text(&self, f: &Fld, n: i128, end: &Json) -> String {
        match (&f.ty, end) {
            (TypeExpr::Date, Json::Str(d)) => d.clone(),
            (TypeExpr::Num(_), _) => amount_text(&f.ty, n, self.lang),
            _ => grouped(n),
        }
    }

    /// The checks of a field a person fills in, each with its code (`items.price.range`) and its
    /// words: whether it is filled in where the machine asks for it (always, while it is shown, or
    /// while its `required when` holds; a check box and rows always hold a value), its length or its
    /// count of rows, its value among a choice's values, its range. `at` is the rows it is a field of.
    fn field_checks(&self, f: &Fld, at: Option<&Fld>) -> Vec<(String, String)> {
        let code = |what: &str| match at {
            Some(r) => format!("{}.{}.{what}", r.named.name, f.named.name),
            None => format!("{}.{what}", f.named.name),
        };
        let l = self.field_words(f, at);
        // the label at the start of a Japanese sentence
        let lj = in_ja(&l, false);
        let mut out = Vec::new();
        let asked = f.asked(at.is_some()) || (f.given() && f.required_when.is_some());
        if asked && !matches!(f.ty, TypeExpr::Bool | TypeExpr::Rows) {
            let words = match f.ty {
                TypeExpr::Choice | TypeExpr::ChoiceFrom(_) | TypeExpr::Person | TypeExpr::Team => self.t(&format!("Choose {l}."), &format!("{lj}を選んでください。")),
                TypeExpr::Attachment => self.t(&format!("Attach {l}."), &format!("{lj}を添付してください。")),
                _ => self.t(&format!("Fill in {l}."), &format!("{lj}を入力してください。")),
            };
            out.push((code("required"), words));
        }
        // a value from outside the options a picker offers (the data a screen is given): a master's
        // rows are the server's, and no screen knows them
        if matches!(f.ty, TypeExpr::Choice | TypeExpr::ChoiceFrom(_)) && !f.choices.is_empty() {
            out.push((code("choice"), self.t(&format!("Choose {l} from its options."), &format!("{lj}は、選択肢から選んでください。"))));
        }
        let (lo, hi) = (f.count_min.filter(|n| *n > 0), f.count_max);
        match (&f.ty, lo, hi) {
            (TypeExpr::Rows, lo, hi) if lo.is_some() || hi.is_some() => {
                let words = match (lo, hi) {
                    (Some(a), Some(b)) => self.t(&format!("Enter {a} to {b} rows of {l}."), &format!("{lj}は{a}行から{b}行までにしてください。")),
                    (Some(a), None) => self.t(&format!("Enter {a} or more rows of {l}."), &format!("{lj}は{a}行以上にしてください。")),
                    (None, b) => {
                        let b = b.unwrap_or(0);
                        self.t(&format!("Enter up to {b} rows of {l}."), &format!("{lj}は{b}行までにしてください。"))
                    }
                };
                out.push((code("count"), words));
            }
            (TypeExpr::Text | TypeExpr::LongText, lo, hi) if lo.is_some() || hi.is_some() => {
                let words = match (lo, hi) {
                    (Some(a), Some(b)) => self.t(&format!("Write {l} in {a} to {} characters.", grouped(b)), &format!("{lj}は{a}文字以上{}文字以内で入力してください。", grouped(b))),
                    (Some(a), None) => self.t(&format!("Write {l} in {a} characters or more."), &format!("{lj}は{a}文字以上で入力してください。")),
                    (None, b) => {
                        let b = grouped(b.unwrap_or(0));
                        self.t(&format!("Write {l} in {b} characters or fewer."), &format!("{lj}は{b}文字以内で入力してください。"))
                    }
                };
                out.push((code("count"), words));
            }
            _ => {}
        }
        if matches!(f.ty, TypeExpr::Num(_) | TypeExpr::Date) && !f.computed && (f.min.is_some() || f.max.is_some()) {
            out.push((code("range"), self.range_words(f, &l, false)));
        }
        out
    }

    /// How a check names the field `f`: by its label, and a field of the rows `at` with the rows'
    /// label too (`Price (Items)`, `明細の金額`).
    fn field_words(&self, f: &Fld, at: Option<&Fld>) -> String {
        match at {
            Some(r) if self.ja() => format!("{}の{}", in_ja(&self.label(&r.named), false), in_ja(&self.label(&f.named), true).trim_end()),
            Some(r) => format!("{} ({})", self.label(&f.named), self.label(&r.named)),
            None => self.label(&f.named),
        }
    }

    /// The words of a value of `f` (labelled `l`) outside its range: one the requester gives, or
    /// one the form works out (`worked`).
    fn range_words(&self, f: &Fld, l: &str, worked: bool) -> String {
        let end = |n: Option<i128>| n.map(|n| self.value_text(f, n, &f_end(f, n)));
        let (lo, hi) = (end(f.min), end(f.max));
        let (en, ja) = match (lo, hi) {
            (Some(a), Some(b)) => (format!("from {a} to {b}"), format!("{a}から{b}まで")),
            (Some(a), None) => (format!("{a} or more"), format!("{a}以上")),
            (None, Some(b)) => (format!("{b} or less"), format!("{b}以下")),
            (None, None) => (String::new(), String::new()),
        };
        if worked {
            self.t(&format!("The {l} the form works out is outside its range ({en})."), &format!("計算した{}が、範囲（{ja}）の外です。", in_ja(l, true)))
        } else {
            self.t(&format!("Enter {l} {en}."), &format!("{}は{ja}にしてください。", in_ja(l, false)))
        }
    }

    /// Every check of the form, with its words: each field's, a row's for every row, then each
    /// computation held to its range (DESIGN 4.4). With `theirs`, the fields only the approvers
    /// give too, in the order of the form: what an approver's edit is held to.
    fn form_checks(&self, theirs: bool) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for f in self.k.form.iter().filter(|f| f.given() || (theirs && f.approvers_only)) {
            out.extend(self.field_checks(f, None));
            for c in f.fields.iter().filter(|c| c.given()) {
                out.extend(self.field_checks(c, Some(f)));
            }
        }
        for h in self.k.screen.worked.iter().flat_map(|w| &w.held) {
            let Some(f) = self.k.form.iter().find(|f| f.named.name == h.rows.clone().unwrap_or_else(|| h.field.clone())) else { continue };
            let (f, l) = match &h.rows {
                Some(_) => match f.fields.iter().find(|c| c.named.name == h.field) {
                    Some(c) => (c, self.field_words(c, Some(f))),
                    None => continue,
                },
                None => (f, self.field_words(f, None)),
            };
            // the range, then the fields it is worked out from, as the framework's pages say a
            // value the server refuses for it
            let mut words = self.range_words(f, &l, true);
            if let Some(fix) = self.fix_words(f) {
                words = format!("{words}{}{fix}", self.t(" ", ""));
            }
            out.push((format!("{}.worked", h.whole()), words));
        }
        out
    }

    /// The fields a computed field `f` is worked out from (its `reads`), for a person to change one
    /// of them: `Change Price (Items).`, `明細の金額を直してください。`. Each is named as a check
    /// names it ([`S::field_words`]); the fields of one rows, when they are all it reads, are named
    /// with the rows once: `Change Quantity or Unit price (Lines).`, `明細の数量か単価を直して
    /// ください。` (DESIGN 13.7). None for a field that reads none the form has.
    fn fix_words(&self, f: &Fld) -> Option<String> {
        let mut reads: Vec<(&Fld, Option<&Fld>)> = Vec::new();
        for r in &f.reads {
            let (of, cell) = match r.split_once('.') {
                Some((of, cell)) => (of, Some(cell)),
                None => (r.as_str(), None),
            };
            let Some(top) = self.k.form.iter().find(|x| x.named.name == of) else { continue };
            let read = match cell {
                Some(cell) => match top.fields.iter().find(|x| x.named.name == cell) {
                    Some(c) => (c, Some(top)),
                    None => continue,
                },
                None => (top, None),
            };
            if !reads.iter().any(|(x, _)| std::ptr::eq(*x, read.0)) {
                reads.push(read);
            }
        }
        let same = |a: Option<&Fld>, b: Option<&Fld>| match (a, b) {
            (Some(a), Some(b)) => std::ptr::eq(a, b),
            (a, b) => a.is_none() && b.is_none(),
        };
        // the rows of them all, or none
        let rows = reads.first()?.1.filter(|r| reads.iter().all(|(_, of)| same(*of, Some(r))));
        let names: Vec<String> = reads.iter().map(|(c, of)| if rows.is_some() { self.label(&c.named) } else { self.field_words(c, *of) }).collect();
        let which = or_words(&names, self.lang);
        Some(match rows {
            Some(r) if self.ja() => ja_join(&[&self.label(&r.named), "の", &which, "を直してください。"]),
            None if self.ja() => ja_join(&[&which, "を直してください。"]),
            Some(r) => format!("Change {which} ({}).", self.label(&r.named)),
            None => format!("Change {which}."),
        })
    }

    /// The checks of the field `f` a part shows below itself, on the form at `form`: a field of rows
    /// says it has too few rows only once the form is sent (`filling`), not while it is filled in
    /// (DESIGN 13.7); the buttons hold the count in full.
    fn own_checks(&self, f: &Fld, form: &Json) -> Vec<Json> {
        self.field_checks(f, None)
            .into_iter()
            .map(|(code, words)| {
                let mut args = vec![("form", form.clone()), ("code", Json::str(&code))];
                if matches!(f.ty, TypeExpr::Rows) && code.ends_with(".count") {
                    args.push(("filling", Json::from(true)));
                }
                rule(self.kcall("check", args), words)
            })
            .collect()
    }

    /// The check of a comment's length (2,000 characters, DESIGN 7.2), as `@checks` writes it.
    fn comment_fits(&self) -> Json {
        rule(self.rcall("commentFits", vec![("text", path("/comment"))]), self.t("Write the comment in 2,000 characters or fewer.", "コメントは2,000文字以内で書いてください。"))
    }

    // ── the parts of a field ──

    /// The source a choice of a master is picked from, as the API serves it (`company_masters/vendors`).
    fn master_of(&self, f: &Fld) -> Option<String> {
        let from = f.type_ascii.strip_prefix("choice from ")?;
        self.k.masters.iter().find(|m| m.named.name == from).map(|m| format!("{}/{}", m.masters, m.master))
    }

    /// The part a requester (or an approver) fills `f` in with, its value at `value`, checked by
    /// `checks`; its ID back. `rows` is the place of the rows a row's part is in.
    fn input(&mut self, f: &Fld, id: &str, value: Json, checks: Vec<Json>) -> String {
        let lbl = Json::str(self.label(&f.named));
        let checked = |mut props: Vec<(&'static str, Json)>| {
            if !checks.is_empty() {
                props.push(("@checks", Json::arr(checks.clone())));
            }
            props
        };
        match &f.ty {
            TypeExpr::Text | TypeExpr::LongText => {
                let variant = if matches!(f.ty, TypeExpr::LongText) { "longText" } else { "shortText" };
                self.add(id, "TextField", checked(vec![("label", lbl), ("value", value), ("variant", Json::str(variant))]))
            }
            TypeExpr::Num(u) => {
                let mut props = vec![("label", lbl), ("value", value), ("decimals", Json::int(decimals(&f.ty)))];
                match (currency(&f.ty), &u.dim) {
                    (Some(c), _) => props.push(("currency", Json::str(c))),
                    (None, ritsu_units::Dim::Rate) => props.push(("unit", Json::str("%"))),
                    (None, _) if !u.unit.is_empty() => props.push(("unit", Json::str(&u.unit))),
                    (None, _) => {}
                }
                // a step that is not one of the last digit (`rate[step 0.25%]`): what a typed
                // amount is counted in
                if step(&f.ty) != 1 {
                    props.push(("step", Json::int(step(&f.ty))));
                }
                if let Some(n) = f.min {
                    props.push(("min", Json::int(n)));
                }
                if let Some(n) = f.max {
                    props.push(("max", Json::int(n)));
                }
                self.radd(id, "AmountField", checked(props))
            }
            TypeExpr::Date => {
                let mut props = vec![("label", lbl), ("value", value), ("enableDate", Json::from(true)), ("enableTime", Json::from(false))];
                if let Some(n) = f.min {
                    props.push(("min", f_end(f, n)));
                }
                if let Some(n) = f.max {
                    props.push(("max", f_end(f, n)));
                }
                self.add(id, "DateTimeInput", checked(props))
            }
            TypeExpr::Choice | TypeExpr::ChoiceFrom(_) if !f.choices.is_empty() => {
                let options = Json::arr(f.choices.iter().map(|c| obj(vec![("label", Json::str(self.label(c))), ("value", Json::str(&c.name))])));
                self.add(id, "ChoicePicker", checked(vec![("label", lbl), ("options", options), ("value", value), ("variant", Json::str("mutuallyExclusive"))]))
            }
            TypeExpr::Choice | TypeExpr::ChoiceFrom(_) => match self.master_of(f) {
                Some(m) => self.radd(id, "MasterPicker", checked(vec![("label", lbl), ("value", value), ("master", Json::str(m)), ("find", path("/find"))])),
                None => self.add(id, "TextField", checked(vec![("label", lbl), ("value", value), ("variant", Json::str("shortText"))])),
            },
            TypeExpr::Attachment => self.radd(id, "FileAttachment", checked(vec![("label", lbl), ("value", value), ("upload", path("/upload"))])),
            TypeExpr::Person | TypeExpr::Team => {
                let of = if matches!(f.ty, TypeExpr::Team) { "team" } else { "person" };
                self.radd(id, "PersonPicker", checked(vec![("label", lbl), ("value", value), ("of", Json::str(of)), ("find", path("/find"))]))
            }
            TypeExpr::Bool => self.add(id, "CheckBox", vec![("label", lbl), ("value", value)]),
            TypeExpr::Rows => {
                let mut row = Vec::new();
                for c in f.fields.iter().filter(|c| c.given()) {
                    let cid = format!("{id}.{}", c.named.name);
                    let checks = self.field_checks(c, Some(f)).into_iter().map(|(code, words)| rule(self.kcall("check", vec![("form", path("/form")), ("code", Json::str(code)), ("row", call("@index", vec![]))]), words)).collect();
                    row.push(self.input(c, &cid, path(&c.named.name), checks));
                }
                let tmpl = self.row(&format!("{id}.row"), row);
                let mut props = vec![("label", lbl), ("value", value), ("child", Json::str(tmpl)), ("addLabel", Json::str(self.t("Add a row", "行を足す"))), ("removeLabel", Json::str(self.t("Remove the row", "この行を消す")))];
                if let Some(n) = f.count_min {
                    props.push(("min", Json::int(n)));
                }
                if let Some(n) = f.count_max {
                    props.push(("max", Json::int(n)));
                }
                self.radd(id, "LineItems", checked(props))
            }
        }
    }

    /// A number of the type of `f`, `value`, formatted: money counted in whole units of its
    /// currency with the basic catalog's `formatCurrency` and a number with no unit with its
    /// `formatNumber`, as they always were; any other number with ritsu's `formatSteps`, which
    /// writes its steps in the unit people count in (`31.25%`, `1.25 USD`; DESIGN 13.1).
    fn formatted(&self, f: &Fld, value: Json) -> Json {
        match currency(&f.ty) {
            _ if !basic_format(&f.ty) => {
                let mut args = vec![("value", value), ("decimals", Json::int(decimals(&f.ty)))];
                if step(&f.ty) != 1 {
                    args.push(("step", Json::int(step(&f.ty))));
                }
                args.push(("unit", Json::str(unit_after(&f.ty, self.lang))));
                self.rcall("formatSteps", args)
            }
            Some(c) => call("formatCurrency", vec![("value", value), ("currency", Json::str(c)), ("decimals", Json::int(decimals(&f.ty)))]),
            None => call("formatNumber", vec![("value", value), ("decimals", Json::int(decimals(&f.ty)))]),
        }
    }

    /// The part that shows the value of `f` at `value`: a code through its label, a number in its
    /// unit, a day as a day, a person and a master's row by their names. Its ID back. A number, a
    /// day and a file are shown while there is one: a format of the basic catalog takes no null
    /// (a renderer shows null as 0), and a file with no name opens nothing.
    fn display(&mut self, f: &Fld, id: &str, value: &str) -> String {
        let v = || path(value);
        let text = match &f.ty {
            TypeExpr::Num(_) => {
                let shown = self.formatted(f, v());
                let t = self.text(id, shown);
                return self.show(&format!("{id}.shown"), v(), t);
            }
            TypeExpr::Date => {
                let t = self.text(id, call("formatDate", vec![("value", v()), ("format", Json::str("yyyy-MM-dd"))]));
                return self.show(&format!("{id}.shown"), v(), t);
            }
            TypeExpr::Choice | TypeExpr::ChoiceFrom(_) if !f.choices.is_empty() => {
                let table = Json::obj(f.choices.iter().map(|c| (c.name.as_str(), Json::str(self.label(c)))));
                self.rcall("lookup", vec![("value", v()), ("table", table)])
            }
            TypeExpr::Choice | TypeExpr::ChoiceFrom(_) => self.rcall("lookup", vec![("value", v()), ("table", path(&format!("/masters/{}", f.named.name)))]),
            TypeExpr::Person | TypeExpr::Team => self.rcall("lookup", vec![("value", v()), ("table", path("/people"))]),
            TypeExpr::Bool => self.rcall("lookup", vec![("value", v()), ("table", obj(vec![("true", Json::str(self.t("Yes", "はい"))), ("false", Json::str(self.t("No", "いいえ")))]))]),
            TypeExpr::Attachment => {
                let name = self.text(&format!("{id}.name"), v());
                let open_text = self.text(&format!("{id}.open.text"), Json::str(self.t("Open the file", "ファイルを開く")));
                // the URL of the file the field holds, by its ID: one table for a row's file too
                let url = self.rcall("lookup", vec![("value", v()), ("table", path("/files"))]);
                let action = obj(vec![("functionCall", call("openUrl", vec![("url", url)]))]);
                let open = self.add(&format!("{id}.open"), "Button", vec![("child", Json::str(open_text)), ("variant", Json::str("borderless")), ("action", action)]);
                let both = self.row(id, vec![name, open]);
                return self.show(&format!("{id}.shown"), v(), both);
            }
            TypeExpr::Rows => {
                let mut cells = Vec::new();
                for c in &f.fields {
                    let cid = format!("{id}.{}", c.named.name);
                    cells.push(self.display(c, &cid, &c.named.name));
                }
                let tmpl = self.row(&format!("{id}.row"), cells);
                return self.add(id, "List", vec![("children", obj(vec![("componentId", Json::str(tmpl)), ("path", Json::str(value))]))]);
            }
            TypeExpr::Text | TypeExpr::LongText => v(),
        };
        self.text(id, text)
    }

    /// A field's label beside the part `shown`, as a card and a form show a value.
    fn labelled(&mut self, f: &Fld, id: &str, shown: String) -> String {
        let l = self.label(&f.named);
        let lbl = self.caption(&format!("{id}.label"), &l);
        self.column(&format!("{id}.labelled"), vec![lbl, shown])
    }

    /// The refusal of the operation a button sent (`/refused`, DESIGN 13.8), at the head of the
    /// screen: shown while there is one that is not of the comment and names no field the screen
    /// places, `unplaced` the fields of the form it does not (on the form, the approvers' fields and
    /// the computations the screens do not work out), so that a refusal of one is not shown nowhere.
    fn refused_head(&mut self, unplaced: &[&Fld]) -> String {
        let named = call("required", vec![("value", path("/refused/field"))]);
        let mut elsewhere = vec![call("not", vec![("value", named)])];
        elsewhere.extend(unplaced.iter().map(|f| self.rcall("equals", vec![("a", path("/refused/field")), ("b", Json::str(&f.named.name))])));
        let elsewhere = if elsewhere.len() == 1 { elsewhere.remove(0) } else { call("or", vec![("values", Json::Arr(elsewhere))]) };
        let when = call("and", vec![("values", Json::arr([call("required", vec![("value", path("/refused/text"))]), elsewhere, call("not", vec![("value", path("/refused/comment"))])]))]);
        let text = self.text("refused.text", path("/refused/text"));
        self.show("refused.shown", when, text)
    }

    /// The refusal of the operation a button sent, shown after the comment while it is one of the
    /// comment (`/refused/comment`: the refusal's kind is `comment`).
    fn refused_comment(&mut self) -> String {
        let text = self.text("comment.refused.text", path("/refused/text"));
        self.show("comment.refused.shown", path("/refused/comment"), text)
    }

    /// The refusal of the operation a button sent, shown after the field `f` while it names the
    /// field (`/refused/field`).
    fn refused_by(&mut self, f: &Fld, id: &str) -> String {
        let when = self.rcall("equals", vec![("a", path("/refused/field")), ("b", Json::str(&f.named.name))]);
        let text = self.text(&format!("{id}.refused.text"), path("/refused/text"));
        self.show(&format!("{id}.refused.shown"), when, text)
    }

    /// `part`, shown only while `f` is shown on the form `form`, when it is shown on a condition.
    fn when_shown(&mut self, f: &Fld, id: &str, part: String, form: Json) -> String {
        if f.shown_when.is_none() {
            return part;
        }
        let when = self.kcall(&format!("shown_{}", f.named.name), vec![("form", form)]);
        self.show(&format!("{id}.shown"), when, part)
    }

    /// A button for the event `e`, shown while `/can/<e>` holds, sending `context` as the event
    /// `name`: the operation of the API it is (`approve`, `submit_expense`; DESIGN 12.4), with the
    /// ticket of its path as `ticket` and its body beside.
    fn button(&mut self, e: &str, name: &str, context: Vec<(&str, Json)>, checks: Vec<Json>, primary: bool) -> String {
        let id = format!("button.{e}");
        let label = self.event_word(e);
        let text = self.text(&format!("{id}.text"), Json::str(label));
        let action = obj(vec![("event", obj(vec![("name", Json::str(name)), ("context", obj(context))]))]);
        let mut props = vec![("child", Json::str(text)), ("variant", Json::str(if primary { "primary" } else { "default" })), ("action", action)];
        if !checks.is_empty() {
            props.push(("@checks", Json::arr(checks)));
        }
        let b = self.add(&id, "Button", props);
        self.show(&format!("{id}.shown"), path(&format!("/can/{e}")), b)
    }

    fn comment_field(&mut self) -> String {
        let checks = vec![self.comment_fits()];
        let lbl = self.t("Comment", "コメント");
        self.add("comment", "TextField", vec![("label", Json::str(lbl)), ("value", path("/comment")), ("variant", Json::str("longText")), ("@checks", Json::arr(checks))])
    }

    fn comment_sent(&self) -> Json {
        self.rcall("comment", vec![("text", path("/comment"))])
    }

    /// What a field holds before anything is put in it: no rows, no choice, nothing.
    fn empty(f: &Fld) -> Json {
        match f.ty {
            TypeExpr::Rows => Json::arr([]),
            TypeExpr::Choice | TypeExpr::ChoiceFrom(_) if !f.choices.is_empty() => Json::arr([]),
            TypeExpr::Bool => Json::from(false),
            _ => Json::Null,
        }
    }

    // ── the five screens ──

    /// The form: whom a proxy makes the request for, the fields the requester gives, what the form
    /// works out, the comment, and the buttons that make the request, make it again and withdraw it.
    /// The data: `requester` and `submit_for` (whom a proxy may make it for), `form`, `before` (on a
    /// request made again, the values the ticket holds), `ticket` (`id`, `revision`,
    /// `sent_back_by`), `comment`, `can`, `people`, `refused` (the refusal of the operation a button
    /// sent), `upload` where the form takes a file (the address the file is handed to), `find` (the
    /// address the pickers find people and rows of masters at), and `may_submit` where `submitted
    /// by` is written.
    fn form(&mut self) -> Json {
        let k = self.k;
        let title_text = self.label(&k.named);
        let title = self.text("title", Json::str(title_text));
        let mut children = vec![title];
        // a proxy makes the request for one of the people it may be made for (DESIGN 5.8), shown
        // while there is someone: none, and it is the person's own
        let for_label = self.t("On behalf of (empty for your own request)", "代わりに申請する相手（自分の申請なら空のまま）");
        let picker = self.radd("requester", "PersonPicker", vec![("label", Json::str(for_label)), ("value", path("/requester")), ("of", Json::str("person")), ("among", path("/submit_for")), ("find", path("/find"))]);
        children.push(self.show("requester.shown", path("/submit_for"), picker));
        let back_label = self.caption("sent_back.label", &self.t("Sent back by", "差し戻した人"));
        let back_name = self.rcall("lookup", vec![("value", path("/ticket/sent_back_by")), ("table", path("/people"))]);
        let back_name = self.text("sent_back.name", back_name);
        let back = self.row("sent_back", vec![back_label, back_name]);
        children.push(self.show("sent_back.shown", path("/ticket/sent_back_by"), back));
        let unplaced: Vec<&Fld> = k.form.iter().filter(|f| !f.given() && !f.worked_on_screen).collect();
        children.push(self.refused_head(&unplaced));
        let mut values = Vec::new();
        for f in &k.form {
            let id = format!("field.{}", f.named.name);
            if f.given() {
                let checks = self.own_checks(f, &path("/form"));
                let part = self.input(f, &id, path(&format!("/form/{}", f.named.name)), checks);
                children.push(self.when_shown(f, &id, part, path("/form")));
                children.push(self.refused_by(f, &id));
                values.push((f.named.name.clone(), Self::empty(f)));
            } else if f.worked_on_screen {
                // shown once the form works it out: its label stays while it cannot
                let value = self.kcall(&f.named.name, vec![("form", path("/form"))]);
                let formatted = self.formatted(f, value.clone());
                let shown = self.text(&format!("{id}.value"), formatted);
                let shown = self.show(&format!("{id}.value.shown"), value, shown);
                let part = self.labelled(f, &id, shown);
                children.push(self.when_shown(f, &id, part, path("/form")));
                // a refusal of what the form works out (out of its range), beside it
                children.push(self.refused_by(f, &id));
            }
        }
        children.push(self.comment_field());
        children.push(self.refused_comment());
        // the buttons: every check of the form holds before the request is made
        let mut checks: Vec<Json> = self.form_checks(false).into_iter().map(|(code, words)| rule(self.kcall("check", vec![("form", path("/form")), ("code", Json::str(code))]), words)).collect();
        checks.push(self.comment_fits());
        let mut submit_checks = checks.clone();
        if k.screen.submitted_by {
            // the person may make it, or makes it for someone it may be made for
            let may = call("or", vec![("values", Json::arr([path("/may_submit"), call("required", vec![("value", path("/requester"))])]))]);
            // the person's own request: one they make for someone else is the machine's to refuse,
            // in words that name that person (`submit`)
            submit_checks.push(rule(may, self.t("You are not one of the people who may make this request as your own.", "この申請は、自分の申請としては出せません。")));
        }
        let given = self.kcall("given", vec![("form", path("/form"))]);
        let submit = self.button("submit", &format!("submit_{}", self.name()), vec![("requester", path("/requester")), ("form", given), ("comment", self.comment_sent())], submit_checks, true);
        let mut buttons = vec![submit];
        let mut can = vec![("submit", Json::from(true))];
        if k.has("resubmit") {
            let changes = self.kcall("changes", vec![("form", path("/form")), ("before", path("/before"))]);
            let cleared = self.kcall("cleared", vec![("form", path("/form")), ("before", path("/before"))]);
            let ctx = vec![("ticket", path("/ticket/id")), ("revision", path("/ticket/revision")), ("form", changes), ("clear", cleared), ("comment", self.comment_sent())];
            buttons.push(self.button("resubmit", &format!("resubmit_{}", self.name()), ctx, checks, true));
            can.push(("resubmit", Json::from(false)));
        }
        if k.has("discard") {
            let ctx = vec![("ticket", path("/ticket/id")), ("revision", path("/ticket/revision")), ("comment", self.comment_sent())];
            let fits = vec![self.comment_fits()];
            buttons.push(self.button("discard", "discard", ctx, fits, false));
            can.push(("discard", Json::from(false)));
        }
        children.push(self.row("buttons", buttons));
        self.column("root", children);
        let mut data = vec![
            ("requester", Json::Null),
            ("submit_for", Json::arr([])),
            ("form", Json::obj(values)),
            ("before", Json::Null),
            ("ticket", obj(vec![("id", Json::Null), ("revision", Json::Null), ("sent_back_by", Json::Null)])),
            ("comment", Json::str("")),
            ("can", obj(can)),
            ("people", obj(vec![])),
            ("refused", Self::refused_data()),
        ];
        if k.form.iter().any(|f| matches!(f.ty, TypeExpr::Attachment) || f.fields.iter().any(|c| matches!(c.ty, TypeExpr::Attachment))) {
            data.push(("upload", Json::Null));
        }
        if self.finds() {
            data.push(("find", Json::Null));
        }
        if k.screen.submitted_by {
            data.push(("may_submit", Json::from(true)));
        }
        obj(data)
    }

    /// The card: whose request it is (and who made it for them, where a proxy did), the ticket's
    /// place in its route, every field, the fields an approver gives, the comment, and a button for
    /// each event a person makes on the ticket (adding an approver, one for each part of the stage
    /// that waits for one). The data: `ticket` (`id`, `revision`, `status`, `stage`, `requester`,
    /// `made_by`), `form`, `route`, `edit` (the server fills it with the ticket's values of the
    /// fields an approver gives or corrects), `comment`, the values of the events (`send_back`,
    /// `add_approver` with its `parts`, `hand_over`), `can`, the names `people`, `masters`,
    /// `files`, `refused` (the refusal of the operation a button sent), `upload` where an approver
    /// gives a file, and `find` where a picker finds people or rows of masters.
    fn card(&mut self) -> Json {
        let k = self.k;
        let title_text = self.label(&k.named);
        let title = self.text("title", Json::str(title_text));
        let id_text = self.text("ticket.id", path("/ticket/id"));
        let status = self.rcall("lookup", vec![("value", path("/ticket/status")), ("table", self.statuses())]);
        let status = self.text("ticket.status", status);
        let head = self.row("head", vec![title, id_text, status]);
        // whose request it is, and who made it for them where a proxy did
        let rq_label = self.caption("requester.label", &self.t("Requester", "申請者"));
        let rq_name = self.rcall("lookup", vec![("value", path("/ticket/requester")), ("table", path("/people"))]);
        let rq_name = self.text("requester.name", rq_name);
        let by_label = self.caption("made_by.label", &self.t("Made by", "代わりに申請した人"));
        let by_name = self.rcall("lookup", vec![("value", path("/ticket/made_by")), ("table", path("/people"))]);
        let by_name = self.text("made_by.name", by_name);
        let by = self.row("made_by", vec![by_label, by_name]);
        let by = self.show("made_by.shown", path("/ticket/made_by"), by);
        let whose = self.row("whose", vec![rq_label, rq_name, by]);
        // the card places every field of the form, and a refusal of one beside it
        let refused = self.refused_head(&[]);
        let route = self.radd("route", "ApprovalRoute", vec![("stages", path("/route")), ("current", path("/ticket/stage")), ("labels", self.stage_table()), ("ways", self.way_table()), ("people", path("/people"))]);
        let mut children = vec![head, whose, refused, route];
        let mut values = Vec::new();
        for f in &k.form {
            let id = format!("field.{}", f.named.name);
            let shown = self.display(f, &format!("{id}.value"), &format!("/form/{}", f.named.name));
            let part = self.labelled(f, &id, shown);
            children.push(self.when_shown(f, &id, part, path("/form")));
            children.push(self.refused_by(f, &id));
            values.push((f.named.name.clone(), if matches!(f.ty, TypeExpr::Rows) { Json::arr([]) } else { Json::Null }));
        }
        let mut can = Vec::new();
        let mut data_extra = Vec::new();
        // the fields an approver gives or corrects, each shown and checked on the form as the edit
        // leaves it: a field it hides is not edited, and one it asks for is not left empty
        let edited: Vec<&Fld> = k.edited().collect();
        let mut buttons = Vec::new();
        if k.has("edit") && !edited.is_empty() {
            let after = self.kcall("edited", vec![("edit", path("/edit")), ("before", path("/form"))]);
            let mut parts = Vec::new();
            let mut edit_values = Vec::new();
            for f in &edited {
                let id = format!("edit.{}", f.named.name);
                let checks = self.own_checks(f, &after);
                let part = self.input(f, &id, path(&format!("/edit/{}", f.named.name)), checks);
                parts.push(self.when_shown(f, &id, part, after.clone()));
                edit_values.push((f.named.name.clone(), Self::empty(f)));
            }
            let edits = self.kcall("edits", vec![("edit", path("/edit")), ("before", path("/form"))]);
            let cleared = self.kcall("edits_cleared", vec![("edit", path("/edit")), ("before", path("/form"))]);
            let ctx = vec![("ticket", path("/ticket/id")), ("revision", path("/ticket/revision")), ("fields", edits), ("clear", cleared), ("comment", self.comment_sent())];
            // the form as the edit leaves it holds every check of the form, as the machine holds it
            let mut checks: Vec<Json> = self.form_checks(true).into_iter().map(|(code, words)| rule(self.kcall("check", vec![("form", after.clone()), ("code", Json::str(code))]), words)).collect();
            checks.push(self.comment_fits());
            parts.push(self.button("edit", &format!("edit_{}", self.name()), ctx, checks, false));
            let edit = self.column("edit", parts);
            children.push(self.show("edit.shown", path("/can/edit"), edit));
            can.push(("edit", Json::from(false)));
            data_extra.push(("edit", Json::obj(edit_values)));
        }
        children.push(self.comment_field());
        children.push(self.refused_comment());
        for e in CARD_EVENTS.into_iter().filter(|e| k.has(e)) {
            let mut ctx = vec![("ticket", path("/ticket/id")), ("revision", path("/ticket/revision"))];
            let mut checks = vec![self.comment_fits()];
            // a reason the request asks for (`reasoned`): the machine refuses the event with none
            // (DESIGN 7.2)
            if k.reasoned.contains(&e) {
                checks.push(rule(self.rcall("commentGiven", vec![("text", path("/comment"))]), self.t("Write the reason in the comment.", "理由をコメントに書いてください。")));
            }
            match e {
                "send_back" if k.screen.back_to_stages => {
                    ctx.push(("to", self.rcall("one", vec![("values", path("/send_back/to"))])));
                    let mut options = vec![obj(vec![("label", Json::str(self.t("The requester", "申請者"))), ("value", Json::str(""))])];
                    options.extend(k.stages.iter().map(|s| obj(vec![("label", Json::str(self.label(s))), ("value", Json::str(&s.name))])));
                    let lbl = self.t("Send it back to", "差し戻す先");
                    let picker = self.add("send_back.to", "ChoicePicker", vec![("label", Json::str(lbl)), ("options", Json::arr(options)), ("value", path("/send_back/to")), ("variant", Json::str("mutuallyExclusive"))]);
                    children.push(self.show("send_back.to.shown", path("/can/send_back"), picker));
                    data_extra.push(("send_back", obj(vec![("to", Json::arr([]))])));
                }
                "add_approver" => {
                    // a button for each part of the stage that waits for an approver (the server's
                    // list): the one it is added to, by the way its approvers are chosen
                    ctx.push(("approver", path("/add_approver/approver")));
                    ctx.push(("part", path("part")));
                    ctx.push(("comment", self.comment_sent()));
                    checks.push(rule(call("required", vec![("value", path("/add_approver/approver"))]), self.t("Choose the approver to add.", "加える承認者を選んでください。")));
                    let lbl = self.t("Approver to add", "加える承認者");
                    let picker = self.radd("add_approver.approver", "PersonPicker", vec![("label", Json::str(lbl)), ("value", path("/add_approver/approver")), ("of", Json::str("person")), ("find", path("/find"))]);
                    let part_name = self.rcall("lookup", vec![("value", path("part")), ("table", self.way_table())]);
                    let part_name = self.text("add_approver.part", part_name);
                    let button = self.button(e, e, ctx, checks, false);
                    let row = self.row("add_approver.row", vec![part_name, button]);
                    let parts = self.add("add_approver.parts", "List", vec![("children", obj(vec![("componentId", Json::str(row)), ("path", Json::str("/add_approver/parts"))]))]);
                    let both = self.column("add_approver", vec![picker, parts]);
                    children.push(self.show("add_approver.shown", path("/can/add_approver"), both));
                    data_extra.push(("add_approver", obj(vec![("approver", Json::Null), ("parts", Json::arr([]))])));
                    can.push((e, Json::from(false)));
                    continue;
                }
                "hand_over" => {
                    ctx.push(("of", path("/hand_over/of")));
                    ctx.push(("to", path("/hand_over/to")));
                    checks.push(rule(call("required", vec![("value", path("/hand_over/of"))]), self.t("Choose whose place is handed over.", "引き継ぐ前の承認者を選んでください。")));
                    checks.push(rule(call("required", vec![("value", path("/hand_over/to"))]), self.t("Choose who takes it.", "引き継ぐ人を選んでください。")));
                    let (l1, l2) = (self.t("Whose place", "引き継ぐ前の承認者"), self.t("Who takes it", "引き継ぐ人"));
                    let of = self.radd("hand_over.of", "PersonPicker", vec![("label", Json::str(l1)), ("value", path("/hand_over/of")), ("of", Json::str("person")), ("find", path("/find"))]);
                    let to = self.radd("hand_over.to", "PersonPicker", vec![("label", Json::str(l2)), ("value", path("/hand_over/to")), ("of", Json::str("person")), ("find", path("/find"))]);
                    let both = self.column("hand_over", vec![of, to]);
                    children.push(self.show("hand_over.shown", path("/can/hand_over"), both));
                    data_extra.push(("hand_over", obj(vec![("of", Json::Null), ("to", Json::Null)])));
                }
                _ => {}
            }
            ctx.push(("comment", self.comment_sent()));
            buttons.push(self.button(e, e, ctx, checks, e == "approve"));
            can.push((e, Json::from(false)));
        }
        // a comment alone: no revision, and a comment to send
        let given = rule(self.rcall("commentGiven", vec![("text", path("/comment"))]), self.t("Write a comment.", "コメントを書いてください。"));
        buttons.push(self.button("comment", "comment", vec![("ticket", path("/ticket/id")), ("comment", self.comment_sent())], vec![self.comment_fits(), given], false));
        can.push(("comment", Json::from(false)));
        children.push(self.row("buttons", buttons));
        self.column("root", children);
        let mut data = vec![
            ("ticket", obj(vec![("id", Json::Null), ("revision", Json::Null), ("status", Json::Null), ("stage", Json::Null), ("requester", Json::Null), ("made_by", Json::Null)])),
            ("form", Json::obj(values)),
            ("route", Json::arr([])),
            ("comment", Json::str("")),
        ];
        data.extend(data_extra);
        data.extend([("can", obj(can)), ("people", obj(vec![])), ("masters", Self::masters_data(k.form.iter())), ("files", self.files_data()), ("refused", Self::refused_data())]);
        if !edited.is_empty() && edited.iter().any(|f| matches!(f.ty, TypeExpr::Attachment)) {
            data.push(("upload", Json::Null));
        }
        if self.finds() {
            data.push(("find", Json::Null));
        }
        obj(data)
    }

    /// The names of the rows of the masters the choices of `fields` read, by field: empty, the
    /// server's to fill.
    fn masters_data<'f>(fields: impl Iterator<Item = &'f Fld>) -> Json {
        Json::obj(fields.filter(|f| matches!(f.ty, TypeExpr::ChoiceFrom(_)) && f.choices.is_empty()).map(|f| (f.named.name.as_str(), obj(vec![]))))
    }

    /// Where each file the form holds is opened, by the file's ID: the server's to fill.
    fn files_data(&self) -> Json {
        obj(vec![])
    }

    /// The refusal of the operation a button sent, which the server fills when the machine refuses
    /// it: its text in the screen's language, the field of the form it names (the first name of
    /// its `at`), null for none, and whether it is one of the comment (its kind is `comment`).
    fn refused_data() -> Json {
        obj(vec![("text", Json::Null), ("field", Json::Null), ("comment", Json::from(false))])
    }

    /// The part below a list that reads its next page (`/more`: the list holds tickets after the
    /// last of the page): the operation `op` of the list again, with the ID of the page's last
    /// ticket as `before`, and what else it is read with (`kind`).
    fn next_page(&mut self, op: &str, mut context: Vec<(&str, Json)>) -> String {
        let label = self.t("Next page", "次のページ");
        let text = self.text("more.text", Json::str(label));
        context.insert(0, ("before", self.rcall("lastOf", vec![("values", path("/tickets")), ("key", Json::str("id"))])));
        let action = obj(vec![("event", obj(vec![("name", Json::str(op)), ("context", obj(context))]))]);
        let button = self.add("more", "Button", vec![("child", Json::str(text)), ("variant", Json::str("default")), ("action", action)]);
        self.show("more.shown", path("/more"), button)
    }

    /// The list of the tickets that wait for the person (`list_waiting`): a line for each ticket,
    /// with its number, state, stage, requester and the short fields of its form, and a button that
    /// opens it; below a full page, a button that reads the next. The data: `tickets` (each with
    /// `id`, `status`, `stage`, `requester`, `form`), `more` (the list's `more`), and the names
    /// `people` and `masters` (of the choices of masters it shows).
    fn list(&mut self) -> Json {
        let title_text = self.label(&self.k.named);
        let title = self.text("title", Json::str(title_text));
        let id_text = self.text("line.id", path("id"));
        let status = self.rcall("lookup", vec![("value", path("status")), ("table", self.statuses())]);
        let status = self.text("line.status", status);
        let stage = self.rcall("lookup", vec![("value", path("stage")), ("table", self.stage_table())]);
        let stage = self.text("line.stage", stage);
        let requester = self.rcall("lookup", vec![("value", path("requester")), ("table", path("/people"))]);
        let requester = self.text("line.requester", requester);
        let mut cells = vec![id_text, status, stage, requester];
        let short: Vec<&Fld> = self.k.given().filter(|f| !matches!(f.ty, TypeExpr::Rows | TypeExpr::LongText | TypeExpr::Attachment) && f.shown_when.is_none()).collect();
        let masters = Self::masters_data(short.iter().copied());
        for f in short {
            let id = format!("line.{}", f.named.name);
            cells.push(self.display(f, &id, &format!("form/{}", f.named.name)));
        }
        let open_text = self.event_word("open");
        let open_text = self.text("line.open.text", Json::str(open_text));
        let action = obj(vec![("event", obj(vec![("name", Json::str("get_ticket")), ("context", obj(vec![("ticket", path("id"))]))]))]);
        let open = self.add("line.open", "Button", vec![("child", Json::str(open_text)), ("variant", Json::str("borderless")), ("action", action)]);
        cells.push(open);
        let line = self.row("line", cells);
        let list = self.add("tickets", "List", vec![("children", obj(vec![("componentId", Json::str(line)), ("path", Json::str("/tickets"))]))]);
        let next = self.next_page("list_waiting", vec![]);
        self.column("root", vec![title, list, next]);
        obj(vec![("tickets", Json::arr([])), ("more", Json::from(false)), ("people", obj(vec![])), ("masters", masters)])
    }

    /// The person's own requests of the kind (`list_mine`): a line for each ticket, with its number,
    /// state and stage, the one who made it for them where a proxy did, and the short fields of its
    /// form, and a button that opens it; below a full page, a button that reads the next, of the
    /// kind. The data: `tickets` (each with `id`, `status`, `stage`, `made_by`, `form`), `more` (the
    /// list's `more`), and the names `people` and `masters` (of the choices of masters it shows).
    fn mine(&mut self) -> Json {
        let kind = self.label(&self.k.named);
        let title_text = if self.ja() { format!("自分の{}", in_ja(&kind, false)) } else { format!("My requests: {kind}") };
        let title = self.text("title", Json::str(title_text));
        let id_text = self.text("line.id", path("id"));
        let status = self.rcall("lookup", vec![("value", path("status")), ("table", self.statuses())]);
        let status = self.text("line.status", status);
        let stage = self.rcall("lookup", vec![("value", path("stage")), ("table", self.stage_table())]);
        let stage = self.text("line.stage", stage);
        let by_words = self.caption("line.made_by.words", &self.t("Made by", "代わりに申請した人"));
        let by_name = self.rcall("lookup", vec![("value", path("made_by")), ("table", path("/people"))]);
        let by_name = self.text("line.made_by.name", by_name);
        let by = self.row("line.made_by", vec![by_words, by_name]);
        let by = self.show("line.made_by.shown", path("made_by"), by);
        let mut cells = vec![id_text, status, stage, by];
        let short: Vec<&Fld> = self.k.given().filter(|f| !matches!(f.ty, TypeExpr::Rows | TypeExpr::LongText | TypeExpr::Attachment) && f.shown_when.is_none()).collect();
        let masters = Self::masters_data(short.iter().copied());
        for f in short {
            let id = format!("line.{}", f.named.name);
            cells.push(self.display(f, &id, &format!("form/{}", f.named.name)));
        }
        let open_text = self.event_word("open");
        let open_text = self.text("line.open.text", Json::str(open_text));
        let action = obj(vec![("event", obj(vec![("name", Json::str("get_ticket")), ("context", obj(vec![("ticket", path("id"))]))]))]);
        let open = self.add("line.open", "Button", vec![("child", Json::str(open_text)), ("variant", Json::str("borderless")), ("action", action)]);
        cells.push(open);
        let line = self.row("line", cells);
        let list = self.add("tickets", "List", vec![("children", obj(vec![("componentId", Json::str(line)), ("path", Json::str("/tickets"))]))]);
        let kind = Json::str(self.name());
        let next = self.next_page("list_mine", vec![("kind", kind)]);
        self.column("root", vec![title, list, next]);
        obj(vec![("tickets", Json::arr([])), ("more", Json::from(false)), ("people", obj(vec![])), ("masters", masters)])
    }

    /// The history: a line for each thing done to the ticket, with when, what, at which stage, by
    /// whom and for whom, the comment, the fields made empty and the note it brought. The data:
    /// `ticket` (`id`), `history` (each with `at`, `did`, `stage`, `by`, `proxy_of`, `comment`,
    /// `cleared`, `note`) and `people`.
    fn history(&mut self) -> Json {
        let kind = self.label(&self.k.named);
        let title_text = if self.ja() { format!("{}の履歴", in_ja(&kind, false)) } else { format!("{kind} history") };
        let title = self.text("title", Json::str(title_text));
        let id_text = self.text("ticket.id", path("/ticket/id"));
        let head = self.row("head", vec![title, id_text]);
        let at = self.text("line.at", call("formatDate", vec![("value", path("at")), ("format", Json::str("yyyy-MM-dd HH:mm"))]));
        let did = self.rcall("lookup", vec![("value", path("did")), ("table", self.dids())]);
        let did = self.text("line.did", did);
        let stage = self.rcall("lookup", vec![("value", path("stage")), ("table", self.stage_table())]);
        let stage = self.text("line.stage", stage);
        let by = self.rcall("lookup", vec![("value", path("by")), ("table", path("/people"))]);
        let by = self.text("line.by", by);
        let first = self.row("line.what", vec![at, did, stage, by]);
        // for whom: `on behalf of <name>`, `<name>の代わりに`
        let whom = self.rcall("lookup", vec![("value", path("proxy_of")), ("table", path("/people"))]);
        let whom = self.text("line.proxy_of.name", whom);
        let proxy = if self.ja() {
            let after = self.text("line.proxy_of.words", Json::str("の代わりに"));
            self.row("line.proxy_of", vec![whom, after])
        } else {
            let before = self.text("line.proxy_of.words", Json::str("on behalf of"));
            self.row("line.proxy_of", vec![before, whom])
        };
        let proxy = self.show("line.proxy_of.shown", path("proxy_of"), proxy);
        let comment = self.text("line.comment", path("comment"));
        let comment = self.show("line.comment.shown", path("comment"), comment);
        let empty_words = self.t("now empty:", "空になった項目:");
        let empty_words = self.caption("line.cleared.words", &empty_words);
        let sep = if self.ja() { "、" } else { ", " };
        let names = self.rcall("lookup", vec![("value", path("cleared")), ("table", self.field_table()), ("separator", Json::str(sep))]);
        let names = self.text("line.cleared.names", names);
        let cleared = self.row("line.cleared", vec![empty_words, names]);
        let cleared = self.show("line.cleared.shown", path("cleared"), cleared);
        let note = self.text("line.note", path("note"));
        let note = self.show("line.note.shown", path("note"), note);
        let line = self.column("line", vec![first, proxy, comment, cleared, note]);
        let list = self.add("history", "List", vec![("children", obj(vec![("componentId", Json::str(line)), ("path", Json::str("/history"))]))]);
        self.column("root", vec![head, list]);
        obj(vec![("ticket", obj(vec![("id", Json::Null)])), ("history", Json::arr([])), ("people", obj(vec![]))])
    }

    /// The notice of a kind no version of which is in effect yet: its title, the day a request may
    /// be made from (the server puts it in `/from`, `YYYY-MM-DD`, which the sentence interpolates;
    /// the sentence is shown while there is one), and a button that opens the server's start (the
    /// server puts its address in `/home`; none, and the button is not shown). The screen sends
    /// nothing (DESIGN 13.8).
    fn offered(&mut self) -> Json {
        let title = self.text("title", Json::str(self.label(&self.k.named)));
        let words = self.t("This request may be made from ${/from}.", "この申請は${/from}から出せます。");
        let notice = self.text("notice", call("formatString", vec![("value", Json::str(&words))]));
        let notice = self.show("notice.shown", path("/from"), notice);
        let home_text = self.text("home.text", Json::str(&self.t("Go to the start page", "ホームへ戻る")));
        let action = obj(vec![("functionCall", call("openUrl", vec![("url", path("/home"))]))]);
        let home = self.add("home", "Button", vec![("child", Json::str(home_text)), ("variant", Json::str("borderless")), ("action", action)]);
        let home = self.show("home.shown", path("/home"), home);
        self.column("root", vec![title, notice, home]);
        obj(vec![("from", Json::Null), ("home", Json::Null)])
    }
}

/// A label set in a Japanese sentence: a space after it where it ends in a letter, a digit or a
/// sign of another script (of ASCII, mostly), and before it where it starts with one and comes
/// `after` Japanese words (`Category を選んでください。`, `計算した Amount が`), as Japanese text
/// sets Latin words apart ([`ja_join`]). A label in Japanese is as it is.
fn in_ja(l: &str, after: bool) -> String {
    let latin = |c: char| side(c) == Side::Other;
    let mut out = String::new();
    if after && l.starts_with(latin) {
        out.push(' ');
    }
    out.push_str(l);
    if l.ends_with(latin) {
        out.push(' ');
    }
    out
}

/// The sides a piece of a Japanese sentence has at an end ([`ja_join`]).
#[derive(Clone, Copy, PartialEq)]
enum Side {
    /// A mark of Japanese text (、。「」（）：), another mark or a space: nothing is set apart from it.
    Mark,
    /// A kana, a kanji, and the full-width letters and digits written with them.
    Japanese,
    /// A letter, a digit or a sign of ASCII, or a letter of another script.
    Other,
}

fn side(c: char) -> Side {
    let japanese = matches!(c,
        '\u{3005}' | '\u{3007}' | '\u{3021}'..='\u{3029}' | '\u{3038}'..='\u{303B}' // 々, 〇 and the kanji's signs
        | '\u{3041}'..='\u{3096}' | '\u{309D}'..='\u{309F}' // hiragana
        | '\u{30A1}'..='\u{30FA}' | '\u{30FC}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' // katakana, ー
        | '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{3FFFF}' // kanji
        | '\u{FF10}'..='\u{FF19}' | '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}' | '\u{FF66}'..='\u{FFDC}'); // full width, half-width kana
    match c {
        _ if c.is_whitespace() => Side::Mark,
        _ if c.is_ascii() => Side::Other,
        _ if japanese => Side::Japanese,
        _ if c.is_alphanumeric() => Side::Other,
        _ => Side::Mark,
    }
}

/// The pieces of a Japanese sentence joined as the framework joins them (its rule of the spaces of
/// Japanese text): one space between a piece that ends in a Japanese letter and one that starts
/// with a letter, a digit or a sign of another script, or the other way round; none next to a mark
/// or a space. `Lines の Quantity か Unit price を直してください。`, `明細の数量か単価を直してください。`.
fn ja_join(pieces: &[&str]) -> String {
    let mut out = String::new();
    for p in pieces.iter().filter(|p| !p.is_empty()) {
        if let (Some(b), Some(a)) = (out.chars().next_back(), p.chars().next()) {
            let (x, y) = (side(b), side(a));
            if x != y && x != Side::Mark && y != Side::Mark {
                out.push(' ');
            }
        }
        out.push_str(p);
    }
    out
}

/// One of `names`, in `lang`: `A`, `A or B`, `A, B or C`; `A`, `AかB`, `A、B、Cのどれか`.
fn or_words(names: &[String], lang: &str) -> String {
    let n = names.len();
    if lang != "ja" {
        return match names {
            [] => String::new(),
            [a] => a.clone(),
            [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
        };
    }
    let mut pieces: Vec<&str> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            pieces.push(if n == 2 { "か" } else { "、" });
        }
        pieces.push(name);
    }
    if n > 2 {
        pieces.push("のどれか");
    }
    ja_join(&pieces)
}

/// An end of a field's range as the screens write it: a day as its text, a number as itself.
fn f_end(f: &Fld, n: i128) -> Json {
    match f.ty {
        TypeExpr::Date => i64::try_from(n).map(|d| Json::str(crate::lex::date_text(d))).unwrap_or(Json::Null),
        _ => Json::int(n),
    }
}
