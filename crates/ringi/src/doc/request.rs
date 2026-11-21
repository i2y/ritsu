//! The page of a request (DESIGN 11.1): the head, then a section for each thing the request decides,
//! in the order a person who approves or manages the requests asks about them.

use super::words::{Say, day_text, grouped};
use super::{Block, h2, h3, id_of};
use crate::ast::{ActionKind, ClaimBody, Ending, Expr, ExprKind, Field, File, Route, Spec, TicketLine, TypeExpr, UseKind, Word};
use crate::check::{Outcome, Run};
use crate::org::Org;
use ritsu_base::text::Lang;
use ritsu_units::Rat;

/// The blocks of the page of the request `o`.
pub fn blocks(o: &Outcome, run: &Run, say: &Say, shown: &str, sha: &str) -> Vec<Block> {
    let mut out = head(say, shown, sha);
    usable(o, say, &mut out);
    submitters(o, say, &mut out);
    form(say, &mut out);
    route_section(say, &mut out);
    ways(say, &mut out);
    deadlines(o, run, say, &mut out);
    rules(say, &mut out);
    comments(say, &mut out);
    proxies(say, &mut out);
    after(say, &mut out);
    claims(o, say, &mut out);
    examples(say, &mut out);
    scenarios(say, &mut out);
    out
}

/// The lines under the title: the file and its version, the team that keeps the request, and what
/// the page reads of the organization.
pub fn head(say: &Say, shown: &str, sha: &str) -> Vec<Block> {
    let f = say.s.file;
    let mut out = Vec::new();
    let (kind_en, kind_ja) = match f.kind {
        crate::ast::FileKind::Request => ("the request", "申請の種類"),
        crate::ast::FileKind::Route => ("the shared route", "共有の経路"),
        crate::ast::FileKind::Organization => ("the organization", "組織"),
        crate::ast::FileKind::Masters => ("the masters", "マスタ"),
    };
    let name = f.name.ascii().to_string();
    let v = f.version;
    out.push(Block::Para(say.t(format!("File: `{shown}` ({kind_en} `{name}`, version {v}, sha256:{sha})."), format!("ファイル：`{shown}`（{kind_ja} `{name}` の版 {v}、sha256:{sha}）"))));
    if let Some(owner) = &f.owner {
        let team = say.team(&owner.text);
        out.push(Block::Para(say.t(format!("Kept by the team {team}."), format!("受け持つチーム：{team}"))));
    }
    if let Some((d, _)) = f.effective_from {
        let d = crate::lex::date_text(d);
        out.push(Block::Para(say.t(format!("This version takes effect on {d}."), format!("この版は {d} から効きます。"))));
    }
    if let Some(org) = say.org {
        let days: Vec<String> = org.versions.iter().map(|v| v.day()).collect();
        let text = match org.at {
            Some(at) => {
                let at = crate::lex::date_text(at);
                say.t(format!("The organization is read in its version in effect on {at}: the one from {}.", days.join(", ")), format!("組織は、{at} に効いている版（{} から）で読んでいます。", days.join("、")))
            }
            None if days.len() == 1 => say.t(format!("The organization is read in its one version, from {}.", days[0]), format!("組織は、{} からの一つの版で読んでいます。", days[0])),
            None => say.t(format!("The organization is read in each of its versions: from {}.", super::words::and_list(&days)), format!("組織は、置いてある版のそれぞれ（{} から）で読んでいます。", days.join("、"))),
        };
        out.push(Block::Para(text));
    } else if let Some(none) = no_org(say) {
        out.push(Block::Note(none));
    }
    out
}

/// What the page says where the request reads an organization with no data: who answers and who may
/// submit are not decided, and how to give the data. None where there is data, or no organization.
fn no_org(say: &Say) -> Option<String> {
    let d = say.s.org_rc()?;
    let crate::org::Data::NoOrg(n) = d.as_ref() else { return None };
    let (line, fix) = (n.line.get(say.lang).to_string(), n.fix.get(say.lang).to_string());
    Some(say.t(
        format!("No data of the organization is read: {line}. This page does not say who answers each stage or who may submit the request. {fix}"),
        format!("組織のデータを読んでいません。{line}。このページは、各ステップにだれが答えるかと、だれが出せるかを言いません。{fix}"),
    ))
}

/// The days the request can be used on (DESIGN 9.6), with the data that decides each end.
pub(super) fn usable(o: &Outcome, say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("When it can be used", "使える期間"), "usable"));
    let Some(u) = &o.usable else {
        out.push(Block::Para(say.t("It can be used on any day: nothing it reads has a first or a last day.", "いつでも使えます。読む物に、始まりの日も終わりの日もありません。")));
        return;
    };
    let mut items = Vec::new();
    match &u.from {
        Some(e) => {
            let (d, by) = (crate::lex::date_text(e.day), crate::usable::said_by(&e.by));
            items.push(say.t(format!("From {d}: {}.", by.en), format!("{d} から使えます。決めているのは、{}です。", by.ja)));
        }
        None => items.push(say.t("From any day: nothing it reads has a first day.", "始まりの日はありません。読む物に始まりの日がありません。")),
    }
    match &u.through {
        Some(e) => {
            let (d, by) = (crate::lex::date_text(e.day), crate::usable::said_by(&e.by));
            items.push(say.t(format!("Through {d}: {}.", by.en), format!("{d} まで使えます。決めているのは、{}です。", by.ja)));
        }
        None => items.push(say.t("With no end: nothing it reads ends.", "終わりの日はありません。読む物に終わりがありません。")),
    }
    out.push(Block::List(items));
    if u.through.is_some() {
        out.push(Block::Para(say.t(
            "Past the last day, new requests, requests made again and the events that start a stage are refused, until the data is extended.",
            "最後の日を過ぎると、データを延ばすまで、新しい申請、出し直し、ステップを始めるイベントは断られます。",
        )));
    }
}

/// Who may submit the request (DESIGN 5.13): the `submitted by` line in words, and how many people of
/// each version of the organization may.
fn submitters(o: &Outcome, say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("Who may submit it", "出せる人"), "submitters"));
    let line = crate::org::submit::of(say.s.file);
    let text = match &line {
        Some(l) => {
            let groups: Vec<String> = l.groups.iter().map(|g| say.group(g)).collect();
            let except: Vec<String> = l.except.iter().map(|g| say.group(g)).collect();
            match except.is_empty() {
                true => say.t(format!("It may be submitted by {}.", super::words::and_list(&groups)), format!("出せるのは、{}です。", groups.join("と"))),
                false => say.t(
                    format!("It may be submitted by {}, except {}.", super::words::and_list(&groups), super::words::and_list(&except)),
                    format!("出せるのは、{}です。ただし、{}は出せません。", groups.join("と"), except.join("と")),
                ),
            }
        }
        None => say.t("Everyone in the organization may submit it.", "組織のだれでも出せます。"),
    };
    out.push(Block::Para(text));
    if let Some(org) = say.org {
        let counts = submit_counts(org, say.s.file, o.usable.as_ref());
        let n = crate::org::of_versions(&counts, Some(("person", "people")));
        out.push(Block::Para(say.t(format!("In numbers: {}.", n.en), format!("人数は、{}です。", n.ja))));
    }
    out.push(Block::Para(say.t(
        "A person may also submit it in the name of someone they stand in for (a proxy for submitting): the request is then that person's.",
        "代わりに出す代理の人は、代わる人の名前で出せます。その申請は、代わられた人の申請になります。",
    )));
}

/// For each version of the organization the request can be used in: its day, how many of its people
/// may submit the request, and how many people it has; the numbers the check says under `ok`.
pub fn submit_counts(org: &Org, f: &File, usable: Option<&crate::usable::Usable>) -> Vec<(String, usize, usize)> {
    let line = crate::org::submit::of(f);
    in_use(org, usable).into_iter().map(|v| (v.day(), v.people.iter().filter(|p| line.as_ref().is_none_or(|l| l.may(org, v, &p.id))).count(), v.people.len())).collect()
}

/// The versions of the organization whose days meet the days the request can be used on: the ones
/// the check reads the request in.
pub fn in_use<'o>(org: &'o Org, usable: Option<&crate::usable::Usable>) -> Vec<&'o crate::org::Version> {
    let from = usable.and_then(|u| u.from.as_ref()).map(|e| e.day);
    let through = usable.and_then(|u| u.through.as_ref()).map(|e| e.day);
    org.versions
        .iter()
        .filter(|v| {
            let (a, b) = org.period(v);
            from.is_none_or(|f| b.is_none_or(|b| b >= f)) && through.is_none_or(|t| a <= t)
        })
        .collect()
}

/// The form: each field, its type, what it accepts, whether it is asked for and when it is shown.
fn form(say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("Form", "フォーム"), "form"));
    let head = vec![say.t("Field", "フィールド"), say.t("Type", "型"), say.t("Accepts", "受け付ける値"), say.t("Required", "必須"), say.t("Shown", "表示")];
    let mut rows = Vec::new();
    for f in &say.s.file.form {
        rows.push(field_row(say, f, None));
        for c in &f.children {
            rows.push(field_row(say, c, Some(f)));
        }
    }
    out.push(Block::Table { head, rows });
    let any_empty = say.s.file.form.iter().flat_map(|f| std::iter::once(f).chain(&f.children)).any(crate::lower::may_be_empty);
    if any_empty {
        out.push(Block::Note(say.t(
            "A field left empty has no value: one that is optional, one asked for or shown on a condition, and one an approver fills in, until they do. A condition that a field “is not” a value holds when it has no value; one that it “is” a value, or that compares it, does not. A computation that reads a field with no value has none either, but a total, which adds the rows whose cell is filled in.",
            "空のフィールドは値を持ちません。任意のフィールド、条件つきで必須か表示のフィールド、承認者が入れるフィールド（入れるまで）がそうです。「〜でない」という条件は、値が無いときも成り立ちます。「〜である」という条件と大きさの比べは、値が無いときは成り立ちません。値の無いフィールドを読む計算も値を持ちません。合計は別で、セルが入っている行だけを足します。",
        )));
    }
    if say.s.file.form.iter().flat_map(|f| std::iter::once(f).chain(&f.children)).any(|f| f.computed.is_some() && f.range.is_some()) {
        out.push(Block::Para(say.t(
            "A computed field accepts what its range says: a request whose value falls outside it is refused, and the route's table needs to cover only that.",
            "計算するフィールドが受け付けるのは、書いた範囲の値です。計算した値が範囲の外になる申請は断られます。経路の表は、受け付ける値を覆えば足ります。",
        )));
    }
}

/// A row of the form's table for the field `f`, of the row of `rows` when it is a field of one.
pub fn field_row(say: &Say, f: &Field, rows: Option<&Field>) -> Vec<String> {
    let path: Vec<&str> = match rows {
        Some(r) => vec![r.name.text.as_str(), f.name.text.as_str()],
        None => vec![f.name.text.as_str()],
    };
    let label = match rows {
        Some(r) => format!("{} › {}", say.field(&[r.name.text.as_str()]), say.field(&path)),
        None => say.field(&path),
    };
    let mut ty = say.ty(f, &path);
    if let Some(e) = &f.computed {
        let c = say.computed(e, rows);
        ty = say.t(format!("{ty}, worked out: {c}"), format!("{ty}。計算：{c}"));
    }
    let accepts = accepts(say, f);
    let required = if f.computed.is_some() {
        say.t("worked out", "計算する")
    } else if f.approvers_only.is_some() {
        if f.optional.is_some() { say.t("filled in by an approver, optional", "承認者が入れる（任意）") } else { say.t("filled in by an approver", "承認者が入れる") }
    } else if let Some(c) = &f.required_when {
        let c = say.cond(c, rows);
        say.when(c)
    } else if f.optional.is_some() || (matches!(f.ty, TypeExpr::Rows) && crate::ports::field(f).lo.is_none_or(|n| n < 1)) {
        // a `rows` field whose count may be 0 is filled in or not
        say.t("no", "任意")
    } else {
        say.t("yes", "必須")
    };
    let shown = match &f.shown_when {
        Some(c) => {
            let c = say.cond(c, rows);
            say.when(c)
        }
        None => say.t("always", "いつも"),
    };
    vec![label, ty, accepts, required, shown]
}

/// What a field accepts: a number's or a date's range (and, for a computed one, what its computation
/// comes to), a text's length, the number of rows.
pub fn accepts(say: &Say, f: &Field) -> String {
    match &f.ty {
        TypeExpr::Num(_) | TypeExpr::Date => {
            let (lo, hi) = crate::lower::words::ends(f);
            let written = f.range.as_ref().map(|_| say.span(&f.ty, lo, hi));
            let reached = if f.computed.is_some() { crate::claims::reach(say.s, f) } else { None };
            match (written, reached) {
                (Some(w), Some((rlo, rhi))) => {
                    let r = say.span(&f.ty, Some(rlo), Some(rhi));
                    if same(lo, Some(rlo)) && same(hi, Some(rhi)) { w } else { say.t(format!("{w} (worked out, it can be {r})"), format!("{w}（計算した値は{r}になりえます）")) }
                }
                (Some(w), None) => w,
                (None, Some((rlo, rhi))) => {
                    let r = say.span(&f.ty, Some(rlo), Some(rhi));
                    say.t(format!("{r}, as it is worked out"), format!("{r}（計算した値）"))
                }
                (None, None) => "—".to_string(),
            }
        }
        TypeExpr::Text | TypeExpr::LongText | TypeExpr::Rows => {
            let rf = crate::ports::field(f);
            let c = say.count(rf.lo, rf.hi, matches!(f.ty, TypeExpr::Rows));
            if c.is_empty() { "—".to_string() } else { c }
        }
        _ => "—".to_string(),
    }
}

fn same(a: Option<Rat>, b: Option<Rat>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp_to(b) == std::cmp::Ordering::Equal,
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// The route: case by case for a table, stage by stage for stage lines, and the shared route a
/// request takes.
pub(super) fn route_section(say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("Route", "経路"), "route"));
    match &say.s.file.route {
        None => out.push(Block::Para(say.t("The request has no route: it is approved as soon as it is submitted.", "経路がありません。出せば、すぐに承認で終わります。"))),
        Some(Route::Table(t)) => {
            let cols: Vec<String> = t.inputs.iter().map(|c| format!("“{}”", say.column(c))).collect();
            let cols_ja: Vec<String> = t.inputs.iter().map(|c| format!("「{}」", say.column(c))).collect();
            out.push(Block::Para(say.t(
                format!("The route depends on {}. Each row is one case, with the stages it goes through, in order; who answers each stage is said under “How the approvers are chosen”.", super::words::and_list(&cols)),
                format!("経路は{}で決まります。表の一行が一つの場合で、通るステップを順に並べています。各ステップにだれが答えるかは「承認者の決め方」にあります。", cols_ja.join("と")),
            )));
            let mut head: Vec<String> = t.inputs.iter().map(|c| say.column(c)).collect();
            head.push(say.t("Route", "経路"));
            let stages: Vec<String> = t.stages.iter().map(|n| n.text.clone()).collect();
            let rows = t
                .rows
                .iter()
                .map(|r| {
                    let mut cells: Vec<String> = t.inputs.iter().zip(&r.tests).map(|(c, (test, _))| say.test(c, test)).collect();
                    cells.push(chain(say, &stages, r.stages.iter().map(|(s, _)| s)));
                    cells
                })
                .collect();
            out.push(Block::Table { head, rows });
            if t.chooses_route {
                for f in say.s.shared_routes().into_iter().flatten() {
                    shared_stages(say, f, &[], out);
                }
            }
        }
        Some(Route::Stages(lines)) => {
            out.push(Block::Para(say.t(
                "The stages, in order. A stage with a condition comes only where it holds; who answers each stage is said under “How the approvers are chosen”.",
                "ステップを順に並べています。条件のあるステップは、条件が成り立つときだけ来ます。各ステップにだれが答えるかは「承認者の決め方」にあります。",
            )));
            let head = vec![say.t("Stage", "ステップ"), say.t("Who answers", "答える人"), say.t("When it comes", "来るとき")];
            let rows = lines
                .iter()
                .map(|l| {
                    let when = match &l.when {
                        Some(c) => {
                            let c = say.cond(c, None);
                            say.when(c)
                        }
                        None => say.t("always", "いつも"),
                    };
                    vec![say.stage(&l.name.text), say.spec(&l.spec), when]
                })
                .collect();
            out.push(Block::Table { head, rows });
        }
        Some(Route::Shared { alias, args, .. }) => {
            let file = say.s.alias_of(&alias.text, UseKind::Route).map(|(u, _)| u.path.clone()).unwrap_or_default();
            let shared = say.s.ringi(&alias.text, UseKind::Route);
            // the route by its own name, which its page and the deadlines below name it by
            let name = shared.map_or(alias.text.as_str(), |f| f.name.text.as_str());
            let given: Vec<String> = args
                .iter()
                .map(|a| {
                    let c = say.cond(&a.value, None);
                    say.t(format!("`{}` holds {}", a.name.text, say.when(c.clone())), format!("経路の `{}` は{}に成り立ちます", a.name.text, say.when(c)))
                })
                .collect();
            let text = match given.is_empty() {
                true => say.t(format!("The request takes the shared route `{name}` (`{file}`)."), format!("共有の経路 `{name}`（`{file}`）を通ります。")),
                false => say.t(
                    format!("The request takes the shared route `{name}` (`{file}`). For the route, {}.", super::words::and_list(&given)),
                    format!("共有の経路 `{name}`（`{file}`）を通ります。{}。", given.join("。")),
                ),
            };
            out.push(Block::Para(text));
            if let Some(f) = shared {
                shared_stages(say, f, args, out);
            }
        }
    }
}

/// The stages of a shared route, with what each condition reads given as the request gives it.
fn shared_stages(say: &Say, f: &File, args: &[crate::ast::Arg], out: &mut Vec<Block>) {
    let text = match say.title_in(f) {
        Some(t) => say.t(format!("The shared route `{}`: {t}", f.name.text), format!("共有の経路 `{}`：{t}", f.name.text)),
        None => say.t(format!("The shared route `{}`", f.name.text), format!("共有の経路 `{}`", f.name.text)),
    };
    out.push(h3(text, id_of("route", &f.name.text)));
    let head = vec![say.t("Stage", "ステップ"), say.t("Who answers", "答える人"), say.t("When it comes", "来るとき")];
    let lines: Vec<&crate::ast::StageLine> = match &f.route {
        Some(Route::Stages(ls)) => ls.iter().collect(),
        _ => Vec::new(),
    };
    let rows = lines
        .iter()
        .map(|l| {
            let when = match &l.when {
                Some(c) => {
                    let c = shared_cond(say, f, c, args);
                    say.when(c)
                }
                None => say.t("always", "いつも"),
            };
            vec![say.stage(&l.name.text), say.spec(&l.spec), when]
        })
        .collect();
    out.push(Block::Table { head, rows });
}

/// A condition of a shared route, said on the request's page: an input of the route is said as the
/// condition the request gives it; `and`, `or` and `not` as they join them.
fn shared_cond(say: &Say, f: &File, c: &Expr, args: &[crate::ast::Arg]) -> String {
    match &c.kind {
        ExprKind::Path(p) if p.len() == 1 && f.inputs.iter().any(|i| i.name.text == p[0].text) => match args.iter().find(|a| a.name.text == p[0].text) {
            Some(a) => say.cond(&a.value, None),
            None => say.t(format!("`{}` is yes", p[0].text), format!("`{}` が「はい」", p[0].text)),
        },
        ExprKind::And(a, b) => {
            let (a, b) = (shared_cond(say, f, a, args), shared_cond(say, f, b, args));
            say.t(format!("{a} and {b}"), format!("{a}、かつ{b}"))
        }
        ExprKind::Or(a, b) => {
            let (a, b) = (shared_cond(say, f, a, args), shared_cond(say, f, b, args));
            say.t(format!("{a}, or {b}"), format!("{a}か、{b}"))
        }
        ExprKind::Not(a) => {
            let a = shared_cond(say, f, a, args);
            say.t(format!("it is not so that {a}"), format!("「{a}」でない"))
        }
        ExprKind::Bool(true) => say.t("always", "いつも"),
        ExprKind::Bool(false) => say.t("never", "どのときも成り立たない"),
        ExprKind::Path(p) => format!("`{}`", crate::ast::dotted(p)),
        _ => say.t("a condition of the shared route", "共有の経路の条件"),
    }
}

/// The stages of a row of a table, in order, as `上長（一次承認）→ 部長と経理（二次承認）`: each
/// stage that comes, who answers it, the stage in brackets.
pub fn chain<'x>(say: &Say, stages: &[String], specs: impl Iterator<Item = &'x Spec>) -> String {
    let parts: Vec<String> = stages
        .iter()
        .zip(specs)
        .filter(|(_, sp)| !matches!(sp, Spec::Empty))
        .map(|(st, sp)| {
            let (who, stage) = (say.spec(sp), say.stage(st));
            match sp {
                Spec::Route(_) => who,
                _ => say.t(format!("{who} ({stage})"), format!("{who}（{stage}）")),
            }
        })
        .collect();
    if parts.is_empty() { say.t("no stage: approved when submitted", "ステップ無し（出せば承認）") } else { parts.join(" → ") }
}

/// How each way to choose an approver chooses (DESIGN 5.2), the request's and its shared routes'.
pub(super) fn ways(say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("How the approvers are chosen", "承認者の決め方"), "ways"));
    let head = vec![say.t("Approver", "承認者"), say.t("Who it is", "決まる人")];
    let mut rows = Vec::new();
    for f in say.files() {
        for w in &f.who {
            let way = say.way(&w.name.text);
            if rows.iter().any(|r: &Vec<String>| r[0] == way) {
                continue;
            }
            rows.push(vec![way, say.how(&w.how)]);
        }
    }
    if rows.is_empty() {
        out.push(Block::Para(say.t("The request names no way to choose an approver.", "承認者の決め方を書いていません。")));
        return;
    }
    out.push(Block::Table { head, rows });
    out.push(Block::Para(match say.s.file.kind {
        // the proxies are a request's: a shared route's page has no section of them
        crate::ast::FileKind::Route => say.t(
            "Each approver is chosen when the stage starts, in the version of the organization in effect then. A proxy in effect that day answers beside them.",
            "承認者は、ステップが始まるときに、そのときに効いている組織の版で決めます。その日に有効な代理も、並んで答えられます。",
        ),
        _ => say.t(
            "Each approver is chosen when the stage starts, in the version of the organization in effect then. A proxy in effect that day answers beside them (see “Proxies”).",
            "承認者は、ステップが始まるときに、そのときに効いている組織の版で決めます。その日に有効な代理も、並んで答えられます（「代理」を見てください）。",
        ),
    }));
}

/// The deadlines and the reminders, with the dates of a stage that starts on an example day, the
/// table a late stage goes up by, and how long a ticket takes at the most (DESIGN 5.10, 7.6).
pub(super) fn deadlines(o: &Outcome, run: &Run, say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("Deadlines and reminders", "期限と催促"), "deadlines"));
    let mut any = false;
    for f in say.files() {
        let shared = !std::ptr::eq(f, say.s.file);
        if let Some(d) = &f.every_stage {
            let head = match shared {
                false => say.t("Every stage:", "どのステップも："),
                true => say.t(format!("Every stage of the shared route `{}`:", f.name.text), format!("共有の経路 `{}` のどのステップも：", f.name.text)),
            };
            out.push(Block::Para(head));
            out.push(Block::List(d.lines.iter().map(|l| say.deadline(l)).collect()));
            any = true;
        }
        if let Some(Route::Stages(lines)) = &f.route {
            for l in lines {
                if let Some(d) = &l.deadlines {
                    let st = say.stage(&l.name.text);
                    out.push(Block::Para(say.t(format!("The stage “{st}”, in place of those:"), format!("ステップ「{st}」は、それに代えて："))));
                    out.push(Block::List(d.lines.iter().map(|x| say.deadline(x)).collect()));
                    any = true;
                }
            }
        }
    }
    if !any {
        out.push(Block::Para(say.t("No stage has a deadline.", "期限のあるステップはありません。")));
    }
    for f in say.files() {
        for t in &f.tables {
            out.push(Block::Para(say.t(format!("A late stage goes up as the table `{}` says:", t.name.text), format!("期限を過ぎたステップは、表 `{}` のとおりに上へ回ります。", t.name.text))));
            // a column named in another script than ASCII reads as a word; an ASCII one as a name
            let named = |n: &crate::ast::Name| if n.alias.is_some() { n.text.clone() } else { format!("`{}`", n.text) };
            let mut head: Vec<String> = t.inputs.iter().map(named).collect();
            head.extend(t.outputs.iter().map(named));
            let ins: Vec<Vec<Word>> = t.inputs.iter().map(|n| vec![Word { text: n.text.clone(), span: n.span }]).collect();
            let rows = t
                .rows
                .iter()
                .map(|r| {
                    let mut cells: Vec<String> = ins.iter().zip(&r.tests).map(|(c, (test, _))| way_or_test(say, c, test)).collect();
                    cells.extend(r.stages.iter().map(|(s, _)| say.spec(s)));
                    cells
                })
                .collect();
            out.push(Block::Table { head, rows });
        }
    }
    example_dates(o, run, say, out);
    let (count, _, _) = crate::ends::summary(say.s);
    if say.s.file.kind == crate::ast::FileKind::Route {
        // a shared route has stages, and no sending back or turn of the requester's of its own
        if let Some(r) = count.round {
            out.push(Block::Para(say.t(
                format!("Its stages take {} business days at the most, one after another. A request that takes the route adds the times it can be sent back and the requester's turns to submit it again; its page counts them.", grouped(r as i128)),
                format!("この経路のステップは、続けて通って長くても {} 営業日です。経路を使う申請の種類は、差し戻せる回数と、申請者が出し直す日数を足します。申請の種類のページがそれを数えます。", grouped(r as i128)),
            )));
        }
        return;
    }
    if let Some(fact) = count.fact() {
        out.push(Block::Para(say.t(format!("A ticket {}. `ritsu ringi doc --longest` shows the dates of the longest.", fact.en), format!("チケットは、{}。いちばん長い場合の日付は `ritsu ringi doc --longest` が見せます。", fact.ja))));
    }
}

/// A cell of a table a late stage goes up by: the name of a way, said as the way, or a test.
fn way_or_test(say: &Say, col: &[Word], t: &crate::ast::Test) -> String {
    if let crate::ast::Test::Values { values, not: false } = t {
        let ways: Vec<String> = values
            .iter()
            .filter_map(|v| match &v.kind {
                ExprKind::Path(p) if p.len() == 1 => Some(say.way(&p[0].text)),
                _ => None,
            })
            .collect();
        if ways.len() == values.len() {
            return say.t(super::words::or_list(&ways), ways.join("か"));
        }
    }
    say.test(col, t)
}

/// The dates of the deadlines and the reminders of each stage, for a stage that starts on the
/// example day: the first business day the request can be used on that comes before a closed day.
fn example_dates(o: &Outcome, run: &Run, say: &Say, out: &mut Vec<Block>) {
    let Some(lowered) = &o.lowered else { return };
    let m = crate::engine::Machine::new(say.s, &run.suite, lowered, o.usable.as_ref());
    let Some(day) = super::longest::example_day(&m, o) else { return };
    let (count, _, _) = crate::ends::summary(say.s);
    // each stage once, in the order of the route, as it comes with a deadline
    let order: Vec<String> = say.s.stages().unwrap_or_default().iter().map(|n| n.text.clone()).collect();
    let mut rows: Vec<(u32, Vec<String>)> = Vec::new();
    for name in &order {
        let Some(occ) = count.paths.iter().flatten().find(|o| &o.stage == name && o.due.is_some() && !matches!(crate::ends::takes(o), crate::ends::Takes::Nothing)) else { continue };
        let times = match crate::ends::takes(occ) {
            crate::ends::Takes::Up { times, .. } => times,
            _ => 0,
        };
        let mut dues = Vec::new();
        let mut remind = None;
        for up in 0..=times.min(2) {
            match m.clocks(&occ.stage, super::longest::started_on(&m, day), 0, up as u8) {
                Ok((r, d)) => {
                    if up == 0 {
                        remind = r;
                    }
                    dues.push(d);
                }
                Err(_) => dues.push(None),
            }
        }
        let end_of_day = occ.due.is_some_and(|d| d.at == crate::ast::At::EndOfDay);
        let at = |i: Option<i64>, end: bool| i.map(|i| super::longest::instant_text(&m, i, end, say.lang)).unwrap_or_else(|| "—".to_string());
        let mut cells = vec![say.stage(&occ.stage), at(remind, false), at(dues.first().copied().flatten(), end_of_day)];
        for up in 1..=2 {
            cells.push(if times >= up { at(dues.get(up as usize).copied().flatten(), end_of_day) } else { "—".to_string() });
        }
        rows.push((times, cells));
    }
    if rows.is_empty() {
        return;
    }
    // the columns after going up only where a stage goes up
    let most = rows.iter().map(|(t, _)| *t).max().unwrap_or(0).min(2) as usize;
    let rows: Vec<Vec<String>> = rows.into_iter().map(|(_, mut c)| {
        c.truncate(3 + most);
        c
    }).collect();
    let d = day_text(day, say.lang);
    let zone = crate::simulate::offset_text(m.offset);
    out.push(Block::Para(say.t(
        format!("For a stage that starts on {d}, the days the request's calendar closes skipped (times in UTC{zone}):"),
        format!("{d}に始まるステップでは、次のとおりです。申請の種類のカレンダーの休みは数えません（時刻は UTC{zone}）。"),
    )));
    let mut head = vec![say.t("Stage", "ステップ"), say.t("Reminder", "催促"), say.t("Due", "期限")];
    let after = [say.t("Due after going up", "上へ回したあとの期限"), say.t("Due after going up again", "もう一度回したあとの期限")];
    head.extend(after.into_iter().take(most));
    out.push(Block::Table { head, rows });
}

/// The rules of a ticket: approving one's own request, one approver at two stages, sending back,
/// making it again, withdrawing, retracting, pausing, what approvers may correct, and what the team
/// that keeps the request may do.
fn rules(say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("Rules of a ticket", "チケットの決まり"), "rules"));
    let lines: &[TicketLine] = say.s.file.ticket.as_ref().map(|t| t.lines.as_slice()).unwrap_or(&[]);
    let mut items = Vec::new();
    match crate::org::listing::self_approval(say.s.file) {
        false => items.push(say.t(
            "A requester does not approve their own request: they are left out of the approvers of every stage, and so is the one who submitted it in their name.",
            "申請者は、自分の申請を承認しません。どのステップの承認者からも外します。代わりに出した人も同じです。",
        )),
        true => items.push(say.t("A requester who is among the approvers of a stage approves it as anyone does.", "申請者がステップの承認者にいれば、ほかの人と同じく承認します。")),
    }
    match crate::org::listing::once(say.s.file) {
        crate::org::listing::Once::No => {}
        crate::org::listing::Once::Before => items.push(say.t(
            "A stage whose one approver approved the stage right before it is done with that approval.",
            "一人の承認者が、すぐ前のステップを承認していれば、そのステップはその承認で済みます。",
        )),
        crate::org::listing::Once::Last => items.push(say.t(
            "One person who alone answers two or more stages answers once, at the last of them: the earlier stages wait for that approval.",
            "一人で二つ以上のステップに答える人は、最後のステップで一度だけ答えます。前のステップは、その承認を待ちます。",
        )),
    }
    for l in lines {
        match l {
            TicketLine::SendBack { to_requester: true, times, .. } => items.push(say.t(
                format!("An approver may send it back to the requester, {} in all, withdrawals counted. After that, they approve or reject it.", times_text(*times, Lang::En)),
                format!("承認者は、申請者へ差し戻せます。取り下げと合わせて{}までです。使い切ったら、承認か却下を選びます。", times_text(*times, Lang::Ja)),
            )),
            TicketLine::SendBack { to_requester: false, times, .. } => items.push(say.t(
                format!("An approver may send it back to the requester, or to an earlier stage that has answered, {} in all, withdrawals counted. After that, they approve or reject it.", times_text(*times, Lang::En)),
                format!("承認者は、申請者か、答えた前のステップへ差し戻せます。取り下げと合わせて{}までです。使い切ったら、承認か却下を選びます。", times_text(*times, Lang::Ja)),
            )),
            TicketLine::Resubmit { days, calendar, .. } => {
                let n = say.days(*days, calendar.as_ref());
                items.push(say.t(
                    format!("Sent back to the requester, it may be submitted again until the end of the day {n} after; then it expires. Only the fields that change are given again."),
                    format!("申請者へ戻ったら、戻った日から {n}の日の終わりまでに出し直せます。過ぎると期限切れで終わります。出し直しでは、変えるフィールドだけを渡します。"),
                ));
            }
            TicketLine::Withdraw { until: Some(st), .. } => {
                let st = say.stage(&st.text);
                items.push(say.t(
                    format!("The requester may withdraw it until the stage “{st}” answers. Withdrawn, it comes back to them, to be submitted again or dropped."),
                    format!("申請者は、ステップ「{st}」が答えるまで取り下げられます。取り下げたチケットは手元へ戻り、直して出し直すか、やめるかを選びます。"),
                ));
            }
            TicketLine::Withdraw { until: None, .. } => items.push(say.t(
                "The requester may withdraw it until it is decided. Withdrawn, it comes back to them, to be submitted again or dropped.",
                "申請者は、決まるまで取り下げられます。取り下げたチケットは手元へ戻り、直して出し直すか、やめるかを選びます。",
            )),
            TicketLine::Retract { .. } => items.push(say.t(
                "An approver may retract their approval until the next stage answers: the ticket goes back to their stage, whose deadline stays as it was.",
                "承認した人は、次のステップが答えるまで、承認を取り消せます。チケットはそのステップへ戻り、期限は始まったときのままです。",
            )),
            TicketLine::Pause { days, calendar, .. } => {
                let n = say.days(*days, calendar.as_ref());
                items.push(say.t(
                    format!("An approver may pause the ticket, {n} in all: its reminders and deadline stop, and move later by the days it was paused. When the days run out, it goes on by itself."),
                    format!("承認者は、チケットを保留にできます。合わせて {n}までです。保留のあいだ、催促と期限の時計は止まり、止めた営業日だけ後ろへずれます。尽きれば自動で戻ります。"),
                ));
            }
            TicketLine::ApproversEdit { fields, .. } => {
                let names: Vec<String> = fields.iter().map(|w| say.field(&[w.text.as_str()])).collect();
                let (en, ja): (Vec<String>, Vec<String>) = (names.iter().map(|n| format!("“{n}”")).collect(), names.iter().map(|n| format!("「{n}」")).collect());
                items.push(say.t(format!("Approvers may correct {}.", super::words::and_list(&en)), format!("承認者は、{}を直せます。", ja.join("と"))));
            }
            _ => {}
        }
    }
    items.push(say.t("An approver may reject it: it ends rejected, and cannot be submitted again.", "承認者は却下できます。チケットは却下で終わり、出し直せません。"));
    items.push(say.t(
        "The team that keeps the request may add an approver to the stage a ticket is at, and hand one approver's part over to someone else. Neither moves a deadline.",
        "申請の種類を受け持つチームは、今のステップに承認者を足せます。承認者の一人の受け持ちを、ほかの人に引き継ぐこともできます。どちらも期限を動かしません。",
    ));
    out.push(Block::List(items));
}

/// `at most 3 times`.
fn times_text(n: u32, lang: Lang) -> String {
    match lang {
        Lang::En => format!("at most {n} {}", if n == 1 { "time" } else { "times" }),
        Lang::Ja => format!(" {n} 回"),
    }
}

/// Comments (DESIGN 7.2): who may write one, how long, and where it is kept.
fn comments(say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("Comments", "コメント"), "comments"));
    let gates: Vec<String> = say.s.uses_of(UseKind::Gate).map(|u| format!("`{}`", u.path)).collect();
    let who = match gates.is_empty() {
        true => say.t("Who may write one is decided by the server's gate.", "だれが書けるかは、サーバーの gate が決めます。"),
        false => say.t(format!("Who may write one is decided by the gate {}, as for the other operations.", super::words::and_list(&gates)), format!("だれが書けるかは、ほかの操作と同じく、gate {}が決めます。", gates.join("と"))),
    };
    out.push(Block::List(vec![
        say.t(
            "A person may add a comment to what they do (submitting, approving, sending back, rejecting, withdrawing and the others), or write one on its own. One with no comment is the same as one with an empty comment.",
            "人がする操作（申請、承認、差し戻し、却下、取り下げなど）には、コメントを一つ添えられます。コメントだけを書くこともできます。添えなくてもかまいません。",
        ),
        who,
        say.t(
            "A comment is up to 2000 characters. It is kept in the ticket's history, on the line of what the person did, with who wrote it and when, and is not changed after. Writing one on its own changes neither the state of the ticket nor its deadlines.",
            "コメントは 2000 字までです。チケットの履歴の、その人がしたことの行に、書いた人と時刻と一緒に残り、あとから変えません。コメントだけを書いても、チケットの状態と期限は変わりません。",
        ),
        say.t(
            "An approver may reject or send back a request without a reason, but a reason written tells the requester what to correct.",
            "理由を書かずに却下も差し戻しもできますが、書けば、申請者は何を直せばよいかを履歴で読めます。",
        ),
    ]));
}

/// Proxies (DESIGN 5.8): what an approve proxy and a submit proxy may do, how the history keeps it,
/// and the rows of the organization's data for the request.
fn proxies(say: &Say, out: &mut Vec<Block>) {
    out.push(h2(say.t("Proxies", "代理"), "proxies"));
    out.push(Block::List(vec![
        say.t(
            "A proxy for approving answers the stages of the person they stand in for, beside them, on the days the row says. Their answer counts once, in that person's seat. They may also pause and resume, correct the fields approvers correct, and retract an approval they gave.",
            "承認の代理は、行の期間のあいだ、代わる人のステップに並んで答えます。答えは、代わる人の席で一票に数えます。保留と再開、承認者が直すフィールドの直し、自分がした承認の取り消しもできます。",
        ),
        say.t(
            "A proxy for submitting submits requests in the name of the person they stand in for, and may withdraw, submit again and drop them as the requester does.",
            "申請の代理は、代わる人の名前で申請を出します。取り下げ、出し直し、やめることも、申請者と同じくできます。",
        ),
        say.t("Whatever a proxy does is kept in the history with whose place they did it in.", "代理がしたことは、だれの代わりにしたかと一緒に、履歴に残ります。"),
    ]));
    let Some(org) = say.org else { return };
    let name = say.s.file.name.ascii().to_string();
    let rows: Vec<Vec<String>> = org
        .proxies
        .iter()
        .filter(|p| p.covers(&name))
        .map(|p| {
            let kind = match p.kind {
                crate::org::ProxyKind::Approve => say.t("approving", "承認"),
                crate::org::ProxyKind::Submit => say.t("submitting", "申請"),
            };
            vec![say.person(&p.person), say.person(&p.proxy), kind, p.days().get(say.lang).to_string()]
        })
        .collect();
    if rows.is_empty() {
        out.push(Block::Para(say.t("The organization's data has no proxy for this request.", "組織のデータに、この申請の種類の代理はありません。")));
        return;
    }
    out.push(Block::Para(say.t("The proxies the organization's data has for this request:", "組織のデータにある、この申請の種類の代理：")));
    out.push(Block::Table { head: vec![say.t("Person", "代わられる人"), say.t("Proxy", "代わる人"), say.t("For", "種類"), say.t("Days", "期間")], rows });
}

/// What the request does to books and flows (DESIGN 7.4): each `on` line in words.
fn after(say: &Say, out: &mut Vec<Block>) {
    let lines: Vec<&TicketLine> = say.s.file.ticket.as_ref().map(|t| t.lines.iter().filter(|l| matches!(l, TicketLine::On { .. })).collect()).unwrap_or_default();
    out.push(h2(say.t("Books and flows", "帳簿と flow"), "after"));
    if lines.is_empty() {
        out.push(Block::Para(say.t("The request holds nothing in a book and starts no flow when it is submitted or ends.", "出したときと終わったときに、帳簿に何も押さえず、flow も始めません。")));
        return;
    }
    let mut items = Vec::new();
    for l in lines {
        let TicketLine::On { endings, action, call, when, .. } = l else { continue };
        let ends: Vec<String> = endings.iter().map(|(e, _)| ending(say, *e)).collect();
        let target = crate::ast::dotted(&call.target);
        let fields: Vec<String> = call
            .args
            .iter()
            .filter_map(|a| match &a.value.kind {
                ExprKind::Path(p) if p.len() == 1 && say.s.field(&p[0].text).is_some() => Some((a.name.text.clone(), say.field(&[p[0].text.as_str()]))),
                _ => None,
            })
            .map(|(n, f)| say.t(format!("`{n}` from “{f}”"), format!("`{n}` に「{f}」")))
            .collect();
        let what = match action {
            ActionKind::Hold => say.t(format!("it holds `{target}` in the book"), format!("帳簿に `{target}` の仮押さえが入ります")),
            ActionKind::Post => say.t(format!("the hold `{target}` is made final"), format!("仮押さえ `{target}` が確定します")),
            ActionKind::Void => say.t(format!("the hold `{target}` is let go"), format!("仮押さえ `{target}` が取り消されます")),
            ActionKind::Launch => say.t(format!("it starts the flow `{target}`"), format!("flow `{target}` が始まります")),
        };
        let with = if fields.is_empty() { String::new() } else { say.t(format!(", with {}", super::words::and_list(&fields)), format!("（{}）", fields.join("、"))) };
        let cond = when.as_ref().map(|c| say.cond(c, None));
        let text = match cond {
            None => say.t(format!("When it is {}, {what}{with}.", super::words::or_list(&ends)), format!("{}とき、{what}{with}。", ends.join("か、"))),
            Some(c) => say.t(format!("When it is {}, {what}{with}, where {c}.", super::words::or_list(&ends)), format!("{}とき、{c}なら、{what}{with}。", ends.join("か、"))),
        };
        items.push(fix_ja_order(text, &with, say));
    }
    out.push(Block::List(items));
}

/// The Japanese of an `on` line reads with what is given before the verb.
fn fix_ja_order(text: String, with: &str, say: &Say) -> String {
    if !say.ja() || with.is_empty() {
        return text;
    }
    text
}

/// An ending an `on` line acts on, in words.
fn ending(say: &Say, e: Ending) -> String {
    match e {
        Ending::Submitted => say.t("submitted", "出した"),
        Ending::Approved => say.t("approved", "承認された"),
        Ending::SentBack => say.t("sent back", "差し戻された"),
        Ending::Rejected => say.t("rejected", "却下された"),
        Ending::Withdrawn => say.t("withdrawn", "取り下げられた"),
        Ending::Expired => say.t("expired", "期限切れになった"),
    }
}

/// The claims (DESIGN 6.4), each in words; the check holds every one, or the file would not pass.
fn claims(o: &Outcome, say: &Say, out: &mut Vec<Block>) {
    let cs = &say.s.file.claims;
    if cs.is_empty() {
        return;
    }
    out.push(h2(say.t("Claims", "主張"), "claims"));
    out.push(Block::Para(say.t("The check holds each of these: a claim that did not hold would stop the request.", "検査は、どれも成り立つことを確かめています。成り立たない主張があれば、この申請の種類は検査を通りません。")));
    let total = o.ticket.as_ref().and_then(|t| t.time.total);
    let mut items = Vec::new();
    for c in cs {
        let n = &c.name.text;
        let text = match &c.body {
            ClaimBody::OnlyWhen { stage, spec, when } => {
                let (st, sp, w) = (say.stage(&stage.text), say.spec(spec), say.cond(when, None));
                say.t(format!("`{n}`: the stage “{st}” is “{sp}” only when {w}."), format!("`{n}`：ステップ「{st}」が「{sp}」になるのは、{}だけです。", say.when(w.clone())))
            }
            ClaimBody::Possible { stage, spec } => {
                let (st, sp) = (say.stage(&stage.text), say.spec(spec));
                say.t(format!("`{n}`: the stage “{st}” can be “{sp}”."), format!("`{n}`：ステップ「{st}」が「{sp}」になる申請があります。"))
            }
            ClaimBody::RemindsBeforeDue => say.t(format!("`{n}`: every stage reminds its approvers before it is due."), format!("`{n}`：どのステップも、期限の前に承認者へ催促が届きます。")),
            ClaimBody::EndsWithin { days } => match total {
                Some(t) => say.t(format!("`{n}`: every ticket ends within {} business days; at the most it takes {}.", grouped(*days as i128), grouped(t as i128)), format!("`{n}`：どのチケットも {} 営業日のうちに終わります。長くても {} 営業日です。", grouped(*days as i128), grouped(t as i128))),
                None => say.t(format!("`{n}`: every ticket ends within {} business days.", grouped(*days as i128)), format!("`{n}`：どのチケットも {} 営業日のうちに終わります。", grouped(*days as i128))),
            },
        };
        items.push(text);
    }
    out.push(Block::List(items));
}

/// The examples of the route: each row's inputs and the stages it comes to.
fn examples(say: &Say, out: &mut Vec<Block>) {
    let Some(ex) = &say.s.file.examples else { return };
    out.push(h2(say.t("Examples of the route", "経路の例"), "examples"));
    out.push(Block::Para(say.t("The check holds that the route gives each of these requests the stages written beside it.", "検査は、経路がどの例にも、横に書いたステップを与えることを確かめています。")));
    let mut head: Vec<String> = ex.inputs.iter().map(|c| say.column(c)).collect();
    head.push(say.t("Route", "経路"));
    let stages: Vec<String> = ex.stages.iter().map(|w| w.text.clone()).collect();
    let rows = ex
        .rows
        .iter()
        .map(|r| {
            let mut cells: Vec<String> = ex.inputs.iter().zip(&r.values).map(|(c, (e, _))| example_cell(say, c, e)).collect();
            cells.push(chain(say, &stages, r.stages.iter().map(|(s, _)| s)));
            cells
        })
        .collect();
    out.push(Block::Table { head, rows });
}

/// A value of an example's row, in the words of its column.
fn example_cell(say: &Say, col: &[Word], e: &Expr) -> String {
    say.test(col, &crate::ast::Test::Values { values: vec![e.clone()], not: false })
}

/// The scenarios: who submits, the form, and each event with the stage and the state it comes to.
fn scenarios(say: &Say, out: &mut Vec<Block>) {
    let ss = &say.s.file.scenarios;
    if ss.is_empty() {
        return;
    }
    out.push(h2(say.t("Scenarios", "シナリオ"), "scenarios"));
    out.push(Block::Para(say.t("The check runs each of these on the ticket machine, and holds that every event comes to the stage and the state written.", "検査は、どのシナリオもチケットの機械で走らせ、イベントごとに、書いたステップと状態になることを確かめています。")));
    for sc in ss {
        out.push(h3(format!("`{}`", sc.name.text), id_of("scenario", &sc.name.text)));
        if let Some(r) = &sc.requester {
            let who = say.person(&r.text);
            out.push(Block::Para(say.t(format!("Submitted by {who}."), format!("申請者：{who}"))));
        }
        let values: Vec<String> = sc.form.iter().map(|(w, e)| form_value(say, w, e)).collect();
        if !values.is_empty() {
            out.push(Block::List(values));
        }
        let comment = sc.events.iter().any(|e| e.comment.is_some());
        let mut head = vec![say.t("At", "時刻"), say.t("Event", "イベント"), say.t("By", "する人")];
        if comment {
            head.push(say.t("Comment", "コメント"));
        }
        head.push(say.t("Stage after", "そのあとのステップ"));
        head.push(say.t("State", "状態"));
        let rows = sc
            .events
            .iter()
            .map(|e| {
                let at = e.at.map(|(d, t)| format!("{} {}", crate::lex::date_text(d), crate::lex::time_text(t))).unwrap_or_default();
                let ev = event_text(say, e);
                let by = e.by.as_ref().map(|b| say.person(&b.text)).unwrap_or_else(|| "—".to_string());
                let mut cells = vec![at, ev, by];
                if comment {
                    cells.push(e.comment.as_ref().map(|c| say.t(format!("“{}”", c.text), format!("「{}」", c.text))).unwrap_or_default());
                }
                cells.push(e.stage.as_ref().map(|s| say.stage(&s.text)).unwrap_or_else(|| "—".to_string()));
                cells.push(state(say, &e.status.text));
                cells
            })
            .collect();
        out.push(Block::Table { head, rows });
    }
}

/// A value of a scenario's form: the field, and the value in its words.
fn form_value(say: &Say, w: &Word, e: &Expr) -> String {
    let f = super::words::find_field(say.s.fields(), &[w.text.as_str()]);
    let label = say.field(&[w.text.as_str()]);
    let v = match (f, &e.kind) {
        (Some(_), ExprKind::List(rows)) if rows.is_empty() => say.t("no rows", "行無し"),
        (Some(f), ExprKind::List(rows)) => {
            let rs: Vec<String> = rows
                .iter()
                .map(|r| match &r.kind {
                    ExprKind::Record(cells) => cells
                        .iter()
                        .map(|(cw, ce)| {
                            let cf = f.children.iter().find(|c| c.name.text == cw.text);
                            value_text(say, cf, &[w.text.as_str(), cw.text.as_str()], ce)
                        })
                        .collect::<Vec<_>>()
                        .join(if say.ja() { "、" } else { ", " }),
                    _ => say.written(r),
                })
                .collect();
            rs.join(if say.ja() { "／" } else { "; " })
        }
        _ => value_text(say, f, &[w.text.as_str()], e),
    };
    say.t(format!("{label}: {v}"), format!("{label}：{v}"))
}

/// A value of a field, in its words: a choice's text, a number as the screens write it, a day, a
/// person by name, a text in quotes.
fn value_text(say: &Say, f: Option<&Field>, path: &[&str], e: &Expr) -> String {
    match (&e.kind, f) {
        (ExprKind::Path(p), Some(f)) if p.len() == 1 && matches!(f.ty, TypeExpr::Choice) => say.value(path, &p[0].text),
        (ExprKind::Path(p), _) if p.len() == 1 => format!("`{}`", p[0].text),
        (ExprKind::Num(_) | ExprKind::Neg(_), Some(f)) => match crate::lower::words::constant_in(e, &f.ty) {
            Some(v) => say.val(&f.ty, v),
            None => say.written(e),
        },
        (ExprKind::Date(d), _) => crate::lex::date_text(*d),
        (ExprKind::Str(s), _) => say.t(format!("“{s}”"), format!("「{s}」")),
        (ExprKind::Bool(b), _) => say.t(if *b { "yes" } else { "no" }, if *b { "はい" } else { "いいえ" }),
        (ExprKind::Person(id), _) => say.person(id),
        (ExprKind::Team(t), _) => say.team(&t.text),
        (ExprKind::None, _) => say.t("no value", "空"),
        _ => format!("`{}`", say.written(e)),
    }
}

/// An event of a scenario, in words where it is a person's or a clock's, with what it carries as
/// written.
fn event_text(say: &Say, e: &crate::ast::Event) -> String {
    let w = e.event.text.as_str();
    let word = match w {
        "submit" => say.t("submit", "申請"),
        "resubmit" => say.t("submit again", "出し直し"),
        "approve" => say.t("approve", "承認"),
        "reject" => say.t("reject", "却下"),
        "send back" => say.t("send back", "差し戻し"),
        "withdraw" => say.t("withdraw", "取り下げ"),
        "discard" => say.t("drop", "やめる"),
        "retract" => say.t("retract the approval", "承認の取り消し"),
        "pause" => say.t("pause", "保留"),
        "resume" => say.t("resume", "再開"),
        "edit" => say.t("correct", "直し"),
        "add approver" => say.t("add an approver", "承認者を足す"),
        "hand over" => say.t("hand over", "引き継ぎ"),
        "comment" => say.t("comment", "コメント"),
        "remind" => say.t("reminder", "催促"),
        "late" => say.t("the deadline passes", "期限が過ぎる"),
        "resubmit late" => say.t("the time to submit again runs out", "出し直しの期限が過ぎる"),
        "pause ends" => say.t("the pause runs out", "保留の日数が尽きる"),
        "agent answered" => say.t("the agent answers", "エージェントの答え"),
        "note ready" => say.t("the summary is ready", "要約ができる"),
        "flow failed" => say.t("the flow fails", "flow の失敗"),
        _ => format!("`{w}`"),
    };
    if e.args.is_empty() {
        return word;
    }
    let args: Vec<String> = e.args.iter().map(|a| event_arg(say, w, a)).collect();
    let args = args.join(if say.ja() { "、" } else { ", " });
    say.t(format!("{word} ({args})"), format!("{word}（{args}）"))
}

/// What an event of a scenario carries, in words: a field given again or corrected with its value,
/// the stage a ticket is sent back to, the persons of a handing over, the agent's answer.
fn event_arg(say: &Say, event: &str, a: &crate::ast::Arg) -> String {
    let n = a.name.text.as_str();
    let word = match &a.value.kind {
        ExprKind::Path(p) if p.len() == 1 => Some(p[0].text.as_str()),
        _ => None,
    };
    match (event, n, word) {
        ("resubmit" | "edit", _, _) => {
            let f = super::words::find_field(say.s.fields(), &[n]);
            let (label, v) = (say.field(&[n]), value_text(say, f, &[n], &a.value));
            say.t(format!("“{label}”: {v}"), format!("「{label}」：{v}"))
        }
        ("agent answered", "verdict", Some("ok")) => say.t("sure", "確か"),
        ("agent answered", "verdict", Some("doubt")) => say.t("not sure", "確かでない"),
        ("send back", "to", Some(st)) => {
            let st = say.stage(st);
            say.t(format!("to “{st}”"), format!("「{st}」へ"))
        }
        ("add approver", "part", Some(way)) => {
            let way = say.way(way);
            say.t(format!("for “{way}”"), format!("「{way}」の分"))
        }
        ("hand over", "from", _) => {
            let p = value_text(say, None, &[], &a.value);
            say.t(format!("from {p}"), format!("{p}から"))
        }
        ("hand over", "to", _) => {
            let p = value_text(say, None, &[], &a.value);
            say.t(format!("to {p}"), format!("{p}へ"))
        }
        _ => value_text(say, None, &[], &a.value),
    }
}

/// A state of a ticket as a scenario writes it, in words.
fn state(say: &Say, s: &str) -> String {
    match s {
        "in progress" => say.t("in progress", "進行中"),
        "paused" => say.t("paused", "保留"),
        "with requester" => say.t("with the requester", "申請者の手元"),
        "approved" => say.t("approved", "承認"),
        "rejected" => say.t("rejected", "却下"),
        "withdrawn" => say.t("withdrawn", "取り下げ"),
        "expired" => say.t("expired", "期限切れ"),
        _ => format!("`{s}`"),
    }
}
