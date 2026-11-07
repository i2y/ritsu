//! What `ritsu impact` prints: text and Markdown in the look of `ritsu mod report`, and JSON that holds
//! the same Markdown, so that one run gives a pipeline the comment and the route.

use super::{Answered, Fact, Impact, Kind};
use crate::modules::diff::{self, ModDiff, sections_markdown, sections_text};
use crate::modules::report::{self, LIMIT, ROWS, Report, sentence};
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text, plural};
use ritsu_base::tr;

/// The most lines of files the text and the Markdown show; the JSON holds every one.
const SHOWN: usize = 200;

/// The facts that say an answer moved.
const MOVED: [&str; 6] = ["rule_changes", "date_changes", "allow_gained", "allow_lost", "policies_moved", "book_changes"];

fn kind_said(k: Kind, lang: Lang) -> &'static str {
    match (k, lang) {
        (Kind::Spec, Lang::Ja) => "仕様",
        (Kind::Generated, Lang::Ja) => "生成物",
        (Kind::Code, Lang::Ja) => "手書きのコード",
        (Kind::Doc, Lang::Ja) => "文書",
        (Kind::Other, Lang::Ja) => "その他",
        (Kind::Spec, _) => "spec",
        (Kind::Generated, _) => "generated",
        (Kind::Code, _) => "code",
        (Kind::Doc, _) => "doc",
        (Kind::Other, _) => "other",
    }
}

fn how_said(how: &str, lang: Lang) -> &'static str {
    match (how, lang) {
        ("added", Lang::Ja) => "足した",
        ("removed", Lang::Ja) => "消した",
        ("renamed", Lang::Ja) => "名前を変えた",
        (_, Lang::Ja) => "変えた",
        ("added", _) => "added",
        ("removed", _) => "removed",
        ("renamed", _) => "renamed",
        _ => "changed",
    }
}

fn why_unknown(why: &str, lang: Lang) -> &'static str {
    match (why, lang) {
        ("unclaimed", Lang::Ja) => "どの主張も走らず、どの要件もこのファイルを指していません",
        ("untraced", Lang::Ja) => "要件にたどり着かない範囲のファイルです",
        ("unnamed", Lang::Ja) => "どの要件もこのファイルを指していません",
        (_, Lang::Ja) => "要件を読めていません",
        ("unclaimed", _) => "no claim runs it and no requirement names the file",
        ("untraced", _) => "a file of a scope no requirement leads to",
        ("unnamed", _) => "no requirement names the file",
        _ => "the requirements were not read",
    }
}

fn why_unclaimed(why: &str, lang: Lang) -> &'static str {
    match (why, lang) {
        ("not_run", Lang::Ja) => "どの主張も走らせていません",
        ("not_reported", Lang::Ja) => "どの実行もこのファイルを報告していません",
        ("no_statement", Lang::Ja) => "記録に文として載っていない行です（表、定数、タグなど）。主張が走るかを確かめられません",
        ("no_lines", Lang::Ja) => "差分に行がないので（名前を変えただけなど）、主張が走るかを確かめられません",
        (_, Lang::Ja) => "主張の記録がないので、走らないものとして数えています",
        ("not_run", _) => "no claim ran them",
        ("not_reported", _) => "no run reported the file",
        ("no_statement", _) => "the record holds no statement on them (a table, a constant, a tag), so nothing says a claim runs them",
        ("no_lines", _) => "the diff has no line of it (a rename, say), so nothing says a claim runs it",
        _ => "there is no record of the claims, so they count as run by none",
    }
}

impl Impact {
    fn head_name(&self, lang: Lang, q: &str) -> String {
        match &self.head {
            Some(h) => format!("{q}{h}{q}"),
            None => tr!("作業ツリー", "the working tree").get(lang).to_string(),
        }
    }

    fn fact_said(&self, f: &Fact) -> String {
        match f {
            Fact::Count(k) => k.to_string(),
            Fact::Yes(b) => b.to_string(),
        }
    }

    /// The route as one phrase: `route = owner_reads, person_reads = true`.
    fn route_phrase(a: &Answered, lang: Lang, q: &str) -> String {
        let and = if lang == Lang::Ja { "、" } else { ", " };
        a.outputs.iter().map(|(n, v, _, _)| format!("{q}{n} = {v}{q}")).collect::<Vec<_>>().join(and)
    }

    /// The sentence at the top: whether an answer moved, the route and the owners.
    fn conclusion(&self, lang: Lang, q: &str) -> String {
        let which: Vec<String> = MOVED.iter().filter(|m| self.fact(m) > 0).map(|m| format!("{q}{m} {}{q}", self.fact(m))).collect();
        let and = if lang == Lang::Ja { "、" } else { ", " };
        let (not, opaque) = (self.undecided.len(), self.fact("opaque_specs"));
        let moved = if !which.is_empty() {
            let w = which.join(and);
            tr!("答えが動きました（{w}）", "The answers moved ({w})")
        } else if not > 0 {
            tr!("比べられた範囲では答えは動いていません（比べられないものが {not} 件）", "No answer moved among what could be compared ({not} could not be)")
        } else if opaque > 0 {
            let files = plural(opaque as usize, "spec file whose contents are not compared", "spec files whose contents are not compared");
            tr!("比べられる答えは動いていません（中身を比べない仕様のファイルが {opaque} 個変わりました）", "No answer that can be compared moved ({files} changed)")
        } else {
            tr!("答えは動いていません", "No answer moved")
        };
        let route = self.route.as_ref().map(|a| {
            let p = Impact::route_phrase(a, lang, q);
            tr!("行き先は {p}", "route: {p}")
        });
        let owners = if !self.requirements.owners.is_empty() {
            let o = self.requirements.owners.iter().map(|o| format!("{q}{o}{q}")).collect::<Vec<_>>().join(and);
            tr!("持ち主は {o}", "owners: {o}")
        } else if self.requirements.known {
            tr!("名前のある持ち主はいません", "no owner is named")
        } else {
            tr!("要件を読めていないので持ち主は分かりません", "the owners are not known, for no requirements were read")
        };
        let parts: Vec<String> = [Some(moved), route, Some(owners)].into_iter().flatten().map(|t| t.get(lang).to_string()).collect();
        if lang == Lang::Ja { format!("{}。", parts.join("。")) } else { format!("{}.", parts.join("; ")) }
    }

    fn files_line(&self, lang: Lang) -> String {
        let count = |k: Kind| self.files.iter().filter(|f| f.kind == k).count();
        let (s, g, c, d, o) = (count(Kind::Spec), count(Kind::Generated), count(Kind::Code), count(Kind::Doc), count(Kind::Other));
        let (all, lines) = (self.files.len(), self.fact("code_lines") as usize);
        let said = plural(lines, "changed line", "changed lines");
        tr!(
            "{all} 個（仕様 {s}、生成物 {g}、手書きのコード {c}、文書 {d}、その他 {o}）。手書きのコードの変わった行 {lines}",
            "{all} changed ({s} spec, {g} generated, {c} code, {d} doc, {o} other); {said} of code written by hand"
        )
        .get(lang)
        .to_string()
    }

    fn file_tail(f: &super::Changed, lang: Lang) -> String {
        let kind = kind_said(f.kind, lang);
        let mut tail = match f.tool {
            Some(t) if f.kind == Kind::Spec => format!("{kind}, {}", t.word()),
            _ => kind.to_string(),
        };
        if f.kind == Kind::Code {
            let (k, said) = (f.lines, plural(f.lines, "line", "lines"));
            tail.push_str(tr!(", {k} 行", ", {said}").get(lang));
        }
        if let Some(from) = &f.from {
            tail.push_str(tr!(", もとは {from}", ", was {from}").get(lang));
        }
        tail
    }

    /// The diff with only the behavior worth showing, and how many files answer the same.
    fn shown(&self) -> (ModDiff, usize) {
        let keep = |b: &&diff::Behavior| b.shows() || b.result.is_err();
        let behavior: Vec<diff::Behavior> = self.diff.behavior.iter().filter(keep).cloned().collect();
        let same = self.diff.behavior.len() - behavior.len();
        (ModDiff { behavior, ..self.diff.clone() }, same)
    }

    fn same_line(same: usize, lang: Lang) -> String {
        let files = plural(same, "other file answers", "other files answer");
        tr!("ほかの {same} 個のファイルは答えが同じです。", "{files} the same.").get(lang).to_string()
    }

    fn requirements_note(&self, lang: Lang) -> Option<String> {
        self.requirements.note.as_ref().map(|t| t.get(lang).to_string())
    }

    fn requirement_line(j: &Json, q: &str, lang: Lang) -> String {
        let s = |k: &str| j.get(k).and_then(Json::as_str).unwrap_or_default().to_string();
        let name = match j.get("version").and_then(Json::as_int) {
            Some(v) => format!("{} v{v}", s("name")),
            None => s("name"),
        };
        let file = match j.get("line").and_then(Json::as_int) {
            Some(l) => format!("{}:{l}", s("file")),
            None => s("file"),
        };
        let owner = match j.get("owner").and_then(Json::as_str) {
            Some(o) => tr!("持ち主 {q}{o}{q}", "owner {q}{o}{q}"),
            None => tr!("持ち主の名前なし", "no owner named"),
        };
        format!("{q}{name}{q} ({q}{file}{q}), {}", owner.get(lang))
    }

    fn closing(&self, lang: Lang, code: u8) -> String {
        let errors = self.new_errors();
        let not = self.undecided.len();
        let t = match code {
            1 => {
                let e = plural(errors, "error", "errors");
                tr!("head にだけあるエラーが {errors} 件あるので、exit 1 です。", "the head has {e} the base does not, which makes exit 1.")
            }
            2 if not > 0 => tr!("比べられないものか決められないものが {not} 件あるので、exit 2 です。", "{not} could not be compared or decided, which makes exit 2."),
            2 => tr!("経路の規則に事実を渡せないので、exit 2 です。", "the facts could not be given to the route rule, which makes exit 2."),
            _ => tr!("新しいエラーはなく、決められなかったものもありません。", "no new error, and nothing was left undecided."),
        };
        t.get(lang).to_string()
    }

    /// The report as text.
    pub(crate) fn text(&self, lang: Lang, code: u8) -> String {
        let c = diff::colon(lang);
        let e = diff::ends(lang);
        let mut o = format!("ritsu impact{c}{} → {}\n\n{}\n", self.base, self.head_name(lang, ""), self.conclusion(lang, ""));
        o.push_str(&format!("\n{}{c}{}\n", tr!("変わったファイル", "files").get(lang), self.files_line(lang)));
        for f in self.files.iter().take(SHOWN) {
            o.push_str(&format!("  {}{c}{} ({})\n", how_said(f.how, lang), f.path, Impact::file_tail(f, lang)));
        }
        if self.files.len() > SHOWN {
            o.push_str(&format!("  {}\n", diff::more_line(self.files.len() - SHOWN, lang)));
        }
        let rl = diff::require_lines(&self.diff.requires, lang);
        if !rl.is_empty() {
            o.push_str(&format!("require{e}\n"));
            for l in rl {
                o.push_str(&format!("  {l}\n"));
            }
        }
        o.push('\n');
        let (base, head) = (self.base.clone(), self.head_name(lang, ""));
        for (name, side) in [(base, &self.base_check), (head, &self.head_check)] {
            let gap = if name.is_ascii() { " " } else { "" };
            o.push_str(&format!("{}{c}{}\n", tr!("{name}{gap}の検査", "the check of {name}").get(lang), Report::counts_line(side.counts, lang)));
        }
        for (title, ds) in [(tr!("新しい診断", "new diagnostics"), &self.new), (tr!("消えた診断", "diagnostics gone"), &self.gone)] {
            if ds.is_empty() {
                o.push_str(&format!("{}{c}{}\n", title.get(lang), tr!("ありません", "none").get(lang)));
                continue;
            }
            o.push_str(&format!("{}{e}\n", title.get(lang)));
            for d in ds {
                o.push_str(&format!("  {}\n", Report::diag_line(d)));
            }
        }
        if !self.undecided.is_empty() {
            o.push_str(&format!("\n{}{e}\n", tr!("比べられないもの、決められないもの", "could not be compared or decided").get(lang)));
            for (what, why) in &self.undecided {
                o.push_str(&format!("  {what}{c}{}\n", why.get(lang)));
            }
        }
        let (shown, same) = self.shown();
        o.push_str(&sections_text(&shown, lang));
        if same > 0 {
            if shown.behavior.is_empty() && shown.interface.is_empty() && shown.why.is_empty() {
                o.push('\n');
            }
            o.push_str(&format!("{}\n", Impact::same_line(same, lang)));
        }
        o.push_str(&format!("\n{}{e}\n", tr!("要件と持ち主", "requirements and owners").get(lang)));
        if let Some(n) = self.requirements_note(lang) {
            o.push_str(&format!("  {n}\n"));
        } else if self.requirements.moved.is_empty() {
            o.push_str(&format!("  {}\n", tr!("動いた要件はありません", "no requirement moved").get(lang)));
        }
        for j in &self.requirements.moved {
            o.push_str(&format!("  {}\n", Impact::requirement_line(j, "", lang)));
        }
        for (file, why) in &self.requirements.unknown {
            o.push_str(&format!("  {}{c}{file} ({})\n", tr!("要件に届かない", "reaches no requirement").get(lang), why_unknown(why, lang)));
        }
        o.push_str(&format!("\n{}{e}\n", tr!("主張", "claims").get(lang)));
        for l in self.claims_lines(lang, "") {
            o.push_str(&format!("  {l}\n"));
        }
        o.push_str(&format!("\n{}{c}{}\n", tr!("生成物", "generated files").get(lang), self.generated_said(lang, "")));
        for f in self.generated.stale.iter().take(SHOWN) {
            o.push_str(&format!("  {f}\n"));
        }
        o.push_str(&format!("\n{}{e}\n", tr!("事実", "facts").get(lang)));
        for (name, f) in &self.facts {
            o.push_str(&format!("  {name} = {}\n", self.fact_said(f)));
        }
        if let Some(a) = &self.route {
            o.push_str(&format!("\n{} ({}){e}\n", tr!("行き先", "route").get(lang), a.file));
            for (n, v, _, _) in &a.outputs {
                o.push_str(&format!("  {n} = {v}\n"));
            }
        }
        o.push_str(&format!("\n{}\n", self.closing(lang, code)));
        o
    }

    /// The lines under "claims", each with what names a file in `q`.
    fn claims_lines(&self, lang: Lang, q: &str) -> Vec<String> {
        let c = diff::colon(lang);
        let mut o = Vec::new();
        if let Some(n) = &self.claims.note {
            o.push(format!("{}{c}{}", tr!("分かりません", "not known").get(lang), n.get(lang)));
        }
        for (spec, why) in &self.claims.specs {
            if let Some(w) = why {
                o.push(format!("{}{c}{q}{spec}{q}{c}{}", tr!("分かりません", "not known").get(lang), w.get(lang)));
            }
        }
        for (spec, name, status, lines) in &self.claims.touched {
            let at: Vec<String> = lines.iter().map(|l| format!("{q}{}:{}{q}", l.file, l.lines)).collect();
            o.push(format!("{}{c}{q}{spec}{q} \"{name}\" ({status}){c}{}", tr!("触った主張", "touched").get(lang), at.join(", ")));
        }
        for (file, side, lines, k, why) in &self.claims.unclaimed {
            let at = match (lines, side) {
                (Some(l), Some(s)) => format!("{q}{file}:{l}{q} ({s})"),
                _ => format!("{q}{file}{q}"),
            };
            let k = *k;
            let said = plural(k, "line", "lines");
            let n = tr!("{k} 行", "{said}").get(lang).to_string();
            o.push(format!("{}{c}{at}{c}{n}{c}{}", tr!("主張が走らない行", "lines no claim runs").get(lang), why_unclaimed(why, lang)));
        }
        if o.is_empty() {
            o.push(tr!("変更に触れる主張はなく、主張が走らない変更行もありません", "no claim is touched, and every changed line of code is run by one").get(lang).to_string());
        }
        o
    }

    fn generated_said(&self, lang: Lang, q: &str) -> String {
        if !self.generated.stale.is_empty() {
            let k = self.generated.stale.len();
            return tr!("古いものが {k} 個あります（{q}ritsu gen --check{q}）", "{k} stale ({q}ritsu gen --check{q})").get(lang).to_string();
        }
        if self.generated.checked {
            return tr!("古いものはありません（{q}ritsu gen --check{q}）", "none stale ({q}ritsu gen --check{q})").get(lang).to_string();
        }
        self.generated.note.as_ref().map_or_else(|| tr!("確かめていません", "not checked").get(lang).to_string(), |t: &Text| t.get(lang).to_string())
    }

    /// The report as Markdown, to paste in a comment of a pull request: every word with an `@` in
    /// backticks, at most [`LIMIT`] characters.
    pub(crate) fn markdown(&self, lang: Lang, code: u8) -> String {
        let c = diff::colon(lang);
        let mut o = format!("## ritsu impact{c}`{}` → {}\n\n{}\n", self.base, self.head_name(lang, "`"), self.conclusion(lang, "`"));
        if let Some(a) = &self.route {
            o.push_str(&format!("\n| {} | {} |\n|---|---|\n", tr!("行き先", "Route").get(lang), tr!("値", "Value").get(lang)));
            for (n, v, _, _) in &a.outputs {
                o.push_str(&format!("| `{}` | `{}` |\n", diff::cell(n), diff::cell(v)));
            }
        }
        o.push_str(&format!("\n| {} | {} | {} |\n|---|---|---|\n", tr!("検査", "Check").get(lang), tr!("エラー", "Errors").get(lang), tr!("警告", "Warnings").get(lang)));
        let (base, head) = (format!("`{}`", self.base), self.head_name(lang, "`"));
        for (name, side) in [(base, &self.base_check), (head, &self.head_check)] {
            o.push_str(&format!("| {} | {} | {} |\n", sentence(&name, lang), diff::n(side.counts.errors), diff::n(side.counts.warnings)));
        }
        if !self.new.is_empty() {
            o.push_str(&format!("\n### {} ({})\n", tr!("新しい診断", "New diagnostics").get(lang), diff::n(self.new.len())));
            o.push_str(&Report::table_of(&self.new, None, lang));
        }
        if !self.gone.is_empty() {
            o.push_str(&format!(
                "\n<details><summary>{} ({})</summary>\n{}\n</details>\n",
                tr!("消えた診断", "Diagnostics gone").get(lang),
                diff::n(self.gone.len()),
                Report::table_of(&self.gone, None, lang)
            ));
        }
        if !self.undecided.is_empty() {
            o.push_str(&format!("\n### {}\n\n", tr!("比べられないもの、決められないもの", "Could not be compared or decided").get(lang)));
            for (what, why) in self.undecided.iter().take(ROWS) {
                o.push_str(&format!("- `{}`{c}{}\n", diff::cell(what), why.get(lang)));
            }
        }
        o.push_str(&format!("\n### {}\n\n{}\n", tr!("変わったファイル", "Files").get(lang), self.files_line(lang)));
        if !self.files.is_empty() {
            o.push_str(&format!(
                "\n<details><summary>{}</summary>\n\n| {} | {} | {} |\n|---|---|---|\n",
                tr!("一覧", "The list").get(lang),
                tr!("変わり方", "Change").get(lang),
                tr!("ファイル", "File").get(lang),
                tr!("種類", "Kind").get(lang)
            ));
            for f in self.files.iter().take(ROWS) {
                o.push_str(&format!("| {} | `{}` | {} |\n", how_said(f.how, lang), diff::cell(&f.path), diff::cell(&Impact::file_tail(f, lang))));
            }
            if self.files.len() > ROWS {
                o.push_str(&format!("\n{}\n", sentence(&diff::more_line(self.files.len() - ROWS, lang), lang)));
            }
            o.push_str("\n</details>\n");
        }
        let items = diff::require_items(&self.diff.requires, lang);
        if !items.is_empty() {
            o.push_str(&format!("\n- require{}\n", diff::ends(lang)));
            for l in items {
                o.push_str(&format!("  - {l}\n"));
            }
        }
        let (shown, same) = self.shown();
        o.push_str(&sections_markdown(&shown, lang, 3, ROWS));
        if same > 0 {
            o.push_str(&format!("\n{}\n", sentence(&Impact::same_line(same, lang), lang)));
        }
        o.push_str(&format!("\n### {}\n\n", tr!("要件と持ち主", "Requirements and owners").get(lang)));
        if let Some(n) = self.requirements_note(lang) {
            o.push_str(&format!("{}\n\n", sentence(&n, lang)));
        } else if self.requirements.moved.is_empty() {
            o.push_str(&format!("{}\n\n", tr!("動いた要件はありません。", "No requirement moved.").get(lang)));
        }
        for j in self.requirements.moved.iter().take(ROWS) {
            o.push_str(&format!("- {}\n", Impact::requirement_line(j, "`", lang)));
        }
        for (file, why) in self.requirements.unknown.iter().take(ROWS) {
            o.push_str(&format!("- {}{c}`{}` ({})\n", tr!("要件に届かない", "Reaches no requirement").get(lang), diff::cell(file), why_unknown(why, lang)));
        }
        o.push_str(&format!("\n### {}\n\n", tr!("主張", "Claims").get(lang)));
        for l in self.claims_lines(lang, "`") {
            o.push_str(&format!("- {}\n", sentence(&l, lang)));
        }
        o.push_str(&format!("\n**{}**{c}{}\n", tr!("生成物", "Generated files").get(lang), self.generated_said(lang, "`")));
        o.push_str(&format!("\n<details><summary>{}</summary>\n\n| {} | {} |\n|---|---|\n", tr!("事実", "Facts").get(lang), tr!("名前", "Name").get(lang), tr!("値", "Value").get(lang)));
        for (name, f) in &self.facts {
            o.push_str(&format!("| `{name}` | {} |\n", self.fact_said(f)));
        }
        o.push_str("\n</details>\n");
        o.push_str(&format!("\n{}\n", sentence(&self.closing(lang, code), lang)));
        report::fit_for(&diff::guarded(&o), LIMIT, lang, "ritsu impact")
    }

    /// The report as JSON.
    pub(crate) fn to_json(&self, lang: Lang, code: u8) -> Json {
        let counts = |s: &super::Side| Json::obj([("errors", Json::int(s.counts.errors as i128)), ("warnings", Json::int(s.counts.warnings as i128))]);
        let texts = |xs: &[Text]| Json::arr(xs.iter().map(|t| Json::str(t.get(lang))));
        let files = self.files.iter().map(|f| {
            Json::obj([
                ("path", Json::str(f.path.as_str())),
                ("change", Json::str(f.how)),
                ("from", Json::opt_str(f.from.clone())),
                ("kind", Json::str(f.kind.word())),
                ("tool", f.tool.map_or(Json::Null, |t| Json::str(t.word()))),
                ("lines", Json::int(f.lines as i128)),
            ])
        });
        let unknown = self.requirements.unknown.iter().map(|(file, why)| Json::obj([("file", Json::str(file.as_str())), ("why", Json::str(*why))]));
        let claims_specs = self.claims.specs.iter().map(|(spec, why)| Json::obj([("spec", Json::str(spec.as_str())), ("known", Json::Bool(why.is_none())), ("why", why.as_ref().map_or(Json::Null, |t| Json::str(t.get(lang))))]));
        let touched = self.claims.touched.iter().map(|(spec, name, status, lines)| {
            Json::obj([
                ("spec", Json::str(spec.as_str())),
                ("name", Json::str(name.as_str())),
                ("status", Json::str(status.as_str())),
                ("lines", Json::arr(lines.iter().map(|l| Json::obj([("file", Json::str(l.file.as_str())), ("side", Json::str(l.side.as_str())), ("lines", Json::str(l.lines.as_str())), ("how", Json::str(l.how.as_str()))])))),
            ])
        });
        let unclaimed = self.claims.unclaimed.iter().map(|(file, side, lines, k, why)| {
            Json::obj([
                ("file", Json::str(file.as_str())),
                ("side", Json::opt_str(side.clone())),
                ("lines", Json::opt_str(lines.clone())),
                ("count", Json::int(*k as i128)),
                ("why", Json::str(*why)),
            ])
        });
        let facts = self.facts.iter().map(|(name, f)| {
            (
                *name,
                match f {
                    Fact::Count(k) => Json::int(*k),
                    Fact::Yes(b) => Json::Bool(*b),
                },
            )
        });
        let route = self.route.as_ref().map_or(Json::Null, |a| {
            Json::obj([("file", Json::str(a.file.as_str())), ("outputs", Json::obj(a.outputs.iter().map(|(_, _, alias, v)| (alias.as_str(), v.clone()))))])
        });
        let fields = diff::json_fields(&self.diff, lang);
        let of = |k: &str| fields.iter().find(|(n, _)| *n == k).map_or(Json::Null, |(_, v)| v.clone());
        Json::obj([
            ("ritsu", Json::str(env!("CARGO_PKG_VERSION"))),
            ("base", Json::obj([("rev", Json::str(self.base.as_str()))])),
            ("head", Json::obj([("rev", Json::opt_str(self.head.clone()))])),
            ("files", Json::arr(files)),
            (
                "check",
                Json::obj([
                    ("base", counts(&self.base_check)),
                    ("head", counts(&self.head_check)),
                    ("new", Json::arr(self.new.iter().cloned())),
                    ("gone", Json::arr(self.gone.iter().cloned())),
                    ("trouble", Json::obj([("base", texts(&self.base_check.trouble)), ("head", texts(&self.head_check.trouble))])),
                ]),
            ),
            ("behavior", of("behavior")),
            ("interface", Json::obj([("files", of("interface")), ("requires", of("requires"))])),
            (
                "requirements",
                Json::obj([
                    ("known", Json::Bool(self.requirements.known)),
                    ("note", self.requirements.note.as_ref().map_or(Json::Null, |t| Json::str(t.get(lang)))),
                    ("moved", Json::arr(self.requirements.moved.iter().cloned())),
                    ("owners", Json::arr(self.requirements.owners.iter().map(|o| Json::str(o.as_str())))),
                    ("unknown", Json::arr(unknown)),
                    ("why", of("why")),
                ]),
            ),
            (
                "claims",
                Json::obj([
                    ("known", Json::Bool(self.claims.known)),
                    ("note", self.claims.note.as_ref().map_or(Json::Null, |t| Json::str(t.get(lang)))),
                    ("specs", Json::arr(claims_specs)),
                    ("touched", Json::arr(touched)),
                    ("unclaimed", Json::arr(unclaimed)),
                ]),
            ),
            (
                "generated",
                Json::obj([
                    ("checked", Json::Bool(self.generated.checked)),
                    ("stale", Json::arr(self.generated.stale.iter().map(|f| Json::str(f.as_str())))),
                    ("note", self.generated.note.as_ref().map_or(Json::Null, |t| Json::str(t.get(lang)))),
                ]),
            ),
            ("undecided", Json::arr(self.undecided.iter().map(|(what, why)| Json::obj([("what", Json::str(what.as_str())), ("why", Json::str(why.get(lang)))])))),
            ("facts", Json::obj(facts)),
            ("route", route),
            ("markdown", Json::str(self.markdown(lang, code))),
        ])
    }
}
