//! The checked rendering (§1.6).
//!
//! A `.rule` file is already almost markdown, so transcribing its syntax is worth nothing (it
//! would only insert one `|---|` line). The job of this surface is to **add, in human words,
//! the facts the checker knows that do not show on the page**. That a single group word hides
//! 6 of the 47 prefectures is something the person approving cannot verify by eye. The checker
//! knows it.
//!
//! The readers are approvers and the business people who review the PR. Rule authors read the
//! `.rule` file directly, so they are not the audience. If the five message texts of §11 are
//! the surface for the writer, this is the first surface for the approver.
//!
//! **Prohibition**: never write a sentence that is not in the checker's output. Every line
//! must trace back to the source file or to a check result. The moment this surface starts
//! inventing summaries or explanations, it too becomes "a surface that says things it has not
//! proven". Facts state their origin, in two kinds: what `rulec check` verified, and what this
//! rendering counted from the declarations.
//!
//! The reverse direction (markdown to `.rule`) is not built. A one-way principle as strong as
//! §1.4.

use crate::ast::*;
use crate::region;
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// Split one source line into its body and its trailing comment. The comment starts at the
/// first `#` outside a string literal, which is where the parser starts it too, so the two
/// never disagree about where the body ends.
fn split_comment(l: &str) -> (&str, Option<String>) {
    let mut in_str = false;
    for (i, ch) in l.char_indices() {
        match ch {
            '"' => in_str = !in_str,
            '#' if !in_str => return (&l[..i], Some(l[i + 1..].trim().to_string())),
            _ => {}
        }
    }
    (l, None)
}

/// Take only the trailing comment from one line of the source file.
/// This is where the origin of a provisional rounding (the §7.2 convention) reaches the
/// approver's eyes, and where the source a table or a row was transcribed from does (§15.32).
/// It is also the breakwater against §16 item three (a placeholder becoming the authority).
fn trailing_comment(lines: &[&str], line: usize) -> Option<String> {
    let l = lines.get(line.checked_sub(1)?)?;
    split_comment(l).1.filter(|c| !c.is_empty())
}

/// Use the source line as it is. Rebuilding the cells from the AST could let the rendering
/// disagree with the source. To protect "traceability" by construction, the text always comes
/// from the source file.
fn source_cells(lines: &[&str], line: usize) -> Vec<String> {
    let Some(l) = lines.get(line.saturating_sub(1)) else { return Vec::new() };
    let (body, _) = split_comment(l);
    let t = body.trim();
    // A label before the first bar is not a cell; it is shown in the `#` column instead.
    let t = match t.find('|') {
        Some(p) => &t[p..],
        None => t,
    };
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

/// The line number of a table's header line (a `|` line containing `→`).
fn header_line(lines: &[&str], from: usize) -> Option<usize> {
    (from..=lines.len()).find(|&n| {
        let l = lines.get(n - 1).map(|s| s.trim()).unwrap_or("");
        l.starts_with('|') && (l.contains("->") || l.contains('→'))
    })
}

fn md_esc(s: &str) -> String {
    s.replace('|', "\\|")
}

/// `3 値` / `3 values` — English needs the singular for one.
fn n_values(n: usize) -> String {
    if crate::i18n::ja() {
        format!("{n} 値")
    } else if n == 1 {
        "1 value".into()
    } else {
        format!("{n} values")
    }
}

/// `この 3 群` / `These 3 groups <verb>` — the English carries the verb so that number
/// agreement (`This group covers` / `These 3 groups cover`) stays in one place.
fn n_groups(n: usize, verb: &str) -> String {
    if crate::i18n::ja() {
        format!("この {n} グループ")
    } else if n == 1 {
        format!("This group {verb}s")
    } else {
        format!("These {n} groups {verb}")
    }
}

fn ty_text(c: &Checked, name: &str) -> String {
    match c.ty_of(name) {
        Some(Ty::Enum(e)) => {
            let n = c.enums.get(&e).map(|v| v.len()).unwrap_or(0);
            tr!("{e}（{}）", "{e} ({})", n_values(n))
        }
        Some(t) => format!("{t}"),
        None => String::new(),
    }
}

fn range_text(c: &Checked, name: &str) -> String {
    let Some((lo, hi)) = c.ranges.get(name) else { return String::new() };
    let unit = match c.ty_of(name) {
        Some(Ty::Qty { unit, .. }) => unit,
        Some(Ty::Money { cur, .. }) => cur,
        _ => String::new(),
    };
    // What the source wrote as `100万円` must not be shown as `1000000円` (§1.6; the same
    // write-back as §2.1).
    let is_date = matches!(c.ty_of(name), Some(Ty::Date));
    let s = |b: &Option<crate::num::Rat>| match b {
        // A date is held as its day number (§2.1); the approver reads the calendar date the
        // rule wrote, not `16161 〜 22279` (§15.71).
        Some(v) if is_date => {
            let (y, m, d) = crate::types::ord_to_date(*v);
            format!("{y:04}-{m:02}-{d:02}")
        }
        Some(v) => format!("{}{unit}", crate::types::fmt_big_pub(*v)),
        None => "…".into(),
    };
    match (lo, hi) {
        (None, None) => String::new(),
        _ => format!("{} 〜 {}", s(lo), s(hi)),
    }
}

/// Where a column comes from. Lets the approver trace "who decided this value".
fn producer(f: &RuleFile, col: &str) -> String {
    if f.inputs.iter().any(|i| i.name.text == col) {
        return tr!("入力", "Input");
    }
    if f.elements.iter().flat_map(|e| &e.fields).any(|i| i.name.text == col) {
        return tr!("要素の欄", "Field of one element");
    }
    for it in &f.items {
        match it {
            Item::Count(d) if d.name.text == col => return tr!("数え上げ", "Count"),
            Item::Derived(d) if d.name.text == col => return tr!("導出", "Derived value"),
            Item::Define(d) if d.name.text == col => return tr!("定義", "Definition"),
            Item::Table(t) if t.outputs.iter().any(|o| o.name.text == col) => {
                let n = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                return if t.clause { tr!("節 {n} の出力", "Output of clause {n}") } else { tr!("表 {n} の出力", "Output of table {n}") };
            }
            _ => {}
        }
    }
    String::new()
}

/// How the groups cover their enum. **Say only what can be said.**
/// If they do not form a partition, say that they do not.
fn group_fact(c: &Checked, en: &str, names: &[String]) -> String {
    let all: Vec<String> = c.enums.get(en).cloned().unwrap_or_default();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for g in names {
        if let Some((_, ms)) = c.groups.get(g) {
            for m in ms {
                *seen.entry(m.as_str()).or_insert(0) += 1;
            }
        }
    }
    let missing: Vec<&str> =
        all.iter().filter(|v| !seen.contains_key(v.as_str())).map(|s| s.as_str()).collect();
    let dup: Vec<&str> = seen.iter().filter(|(_, n)| **n > 1).map(|(v, _)| *v).collect();
    let n = names.len();
    match (missing.is_empty(), dup.is_empty()) {
        (true, true) => tr!(
            "{g}は {en} の {all}を過不足なく分割しています（この資料が宣言から数えました）。",
            "{g} the {all} of {en} exactly (counted from the declarations by this rendering).",
            g = n_groups(n, "partition"),
            all = n_values(all.len())
        ),
        (true, false) => tr!(
            "{g}は {en} の {all}をすべて覆いますが、{dup} が二つ以上のグループに入っています（この資料が宣言から数えました）。",
            "{g} all {all} of {en}, but the following belong to two or more groups: {dup} (counted from the declarations by this rendering).",
            g = n_groups(n, "cover"),
            all = n_values(all.len()),
            dup = dup.join(sep())
        ),
        (false, true) => tr!(
            "{g}が覆うのは {en} の {all}のうち {seen}で、{miss} はどのグループにも入っていません（この資料が宣言から数えました）。",
            "{g} {seen} of the {all} of {en}; the following belong to no group: {miss} (counted from the declarations by this rendering).",
            g = n_groups(n, "cover"),
            all = n_values(all.len()),
            seen = n_values(seen.len()),
            miss = join_some(&missing, 8)
        ),
        (false, false) => tr!(
            "{g}は {en} の {all}のうち {seen}を覆い、{dup} が重複、{miss} が未収容です（この資料が宣言から数えました）。",
            "{g} {seen} of the {all} of {en}; in two or more groups: {dup}; in no group: {miss} (counted from the declarations by this rendering).",
            g = n_groups(n, "cover"),
            all = n_values(all.len()),
            seen = n_values(seen.len()),
            dup = dup.join(sep()),
            miss = join_some(&missing, 8)
        ),
    }
}

fn join_some(v: &[&str], max: usize) -> String {
    if v.len() <= max {
        return v.join(sep());
    }
    tr!("{}（ほか {} 件）", "{} (and {} more)", v[..max].join(sep()), v.len() - max)
}

/// The list separator of the current output language.
fn sep() -> &'static str {
    if crate::i18n::ja() { "、" } else { ", " }
}

/// Render a rule that passed check. The caller rejects rules that do not pass
/// (a clean rendering of a broken rule would be a lie; §1.6).
pub fn render(f: &RuleFile, c: &Checked, src: &str, path: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let hash = crate::codegen::hash(src);
    let mut o = String::new();

    // The greatest danger is a stale rendering that lives on looking authoritative, so stamp
    // the source path, its hash and the tool version. Staleness can then be checked wherever
    // the document was pasted.
    o.push_str(&tr!(
        "<!-- rulec {} が {path} (sha256:{}) から生成。これは読み取り専用の資料で、\
         本物は .rule のほうです。編集しても戻せません（§1.6）。 -->\n",
        "<!-- Generated by rulec {} from {path} (sha256:{}). This is a read-only rendering; \
         the source of truth is the .rule file. Edits cannot be carried back (§1.6). -->\n",
        env!("CARGO_PKG_VERSION"),
        &hash[..12]
    ));
    o.push_str(&tr!("# 規則 {} v{}\n", "# Rule {} v{}\n", f.name.text, f.version));
    if let Some(d) = &f.description {
        o.push_str(&format!("\n{d}\n"));
    }

    // --- Inputs
    o.push_str(&tr!(
        "\n## 入力\n\n| 名前 | 型 | 範囲 | 注記 |\n|---|---|---|---|\n",
        "\n## Inputs\n\n| Name | Type | Range | Notes |\n|---|---|---|---|\n"
    ));
    for i in &f.inputs {
        let n = &i.name.text;
        let mut note: Vec<String> = Vec::new();
        if i.contract_only {
            note.push(tr!(
                "`{}`（範囲の入口検査にだけ使い、表の条件には現れません）",
                "`{}` (used only for the range check at the entry; it never appears in a table condition)",
                crate::kw::CONTRACT_ONLY
            ));
        }
        if let Some(cm) = trailing_comment(&lines, i.name.span.line) {
            note.push(cm);
        }
        o.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            md_esc(n),
            md_esc(&ty_text(c, n)),
            md_esc(&range_text(c, n)),
            md_esc(&note.join(" / "))
        ));
    }

    // --- The sequence the rule walks, and the fields of one element (§15.56). An approver
    // who cannot see these is reading half the rule: the caller passes them too.
    if let Some(el) = &f.elements {
        o.push_str(&tr!(
            "\n## 順に見ていく並び: {}\n\n一件ぶんの欄です。呼び出し側は、この欄のそろった要素を何件でも渡します。\n\n| 名前 | 型 | 範囲 | 注記 |\n|---|---|---|---|\n",
            "\n## The sequence walked: {}\n\nThe fields of one element. The caller passes any number of elements, each with these fields filled in.\n\n| Name | Type | Range | Notes |\n|---|---|---|---|\n",
            el.name.text
        ));
        for fd in &el.fields {
            let n = &fd.name.text;
            let note = trailing_comment(&lines, fd.name.span.line).unwrap_or_default();
            o.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                md_esc(n),
                md_esc(&ty_text(c, n)),
                md_esc(&range_text(c, n)),
                md_esc(&note)
            ));
        }
    }

    // --- Outputs
    o.push_str(&tr!(
        "\n## 出力\n\n| 名前 | 型 | 丸め | 注記 |\n|---|---|---|---|\n",
        "\n## Outputs\n\n| Name | Type | Rounding | Notes |\n|---|---|---|---|\n"
    ));
    for od in &f.outputs {
        let n = &od.name.text;
        let r = od.rounding.as_ref().map(|r| format!("{}({})", r.mode, r.grid.raw)).unwrap_or_default();
        // The origin of a provisional rounding reaches the approver's eyes only here
        // (§7.2, §16 item three).
        let note = trailing_comment(&lines, od.name.span.line).unwrap_or_default();
        o.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            md_esc(n),
            md_esc(&ty_text(c, n)),
            md_esc(&r),
            md_esc(&note)
        ));
    }

    // --- Types. Closed enums: once a value is added, the completeness check breaks the
    // existing tables.
    if !f.enums.is_empty() || !f.imports.is_empty() || !f.enum_imports.is_empty() {
        o.push_str(&tr!("\n## 型\n\n", "\n## Types\n\n"));
        o.push_str(&tr!(
            "列挙は**閉じた**有限集合です。値を足すと、それを見ていない表が完全性検査で割れます。\n\n",
            "An enum is a **closed** finite set. Add a value, and every table that does not look at it fails the completeness check.\n\n"
        ));
        for e in &f.enums {
            let vs: Vec<String> = e
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    // The `既定扱い` mark of §2.1 (`default`): a declaration that the value has
                    // no row of its own and is meant to fall to the default row. This intent is
                    // exactly what the approver should read.
                    if *e.default_marks.get(i).unwrap_or(&false) {
                        format!("{}（{}）", v.text, crate::kw::DEFAULT)
                    } else {
                        v.text.clone()
                    }
                })
                .collect();
            o.push_str(&tr!(
                "- **{}**（{}）— {}\n",
                "- **{}** ({}) — {}\n",
                md_esc(&e.name.text),
                n_values(e.values.len()),
                md_esc(&vs.join(sep()))
            ));
            // Where the set came from, when it is not this file's to decide (§15.59). The
            // approver is the person who has to know the set can change without this file changing.
            if let Some(p) = f.enum_imports.iter().find(|p| p.target.text == e.name.text) {
                o.push_str(&tr!(
                    "  - この値の集合は `{0}` の `{1}` が持っています。`rulec check` が一致を確かめています（ずれていれば E032、値に行が無ければ E033）。\n",
                    "  - This set is owned by `{1}` in `{0}`. `rulec check` holds the two together (E032 when they differ, E033 when a value has no row).\n",
                    md_esc(&p.file),
                    md_esc(&p.source)
                ));
            }
        }
        for (im, _) in &f.imports {
            let name = im.rsplit('/').next().unwrap_or(im);
            let n = c.enums.get(name).map(|v| v.len()).unwrap_or(0);
            o.push_str(&tr!(
                "- **{}**（{}）— 組み込み（`{} {}`）\n",
                "- **{}** ({}) — built-in (`{} {}`)\n",
                md_esc(name),
                n_values(n),
                crate::kw::IMPORT,
                md_esc(im)
            ));
        }
        if !f.enum_imports.is_empty() {
            o.push_str(&tr!(
                "\n取り込んだ列挙では、行にも現れず `{}` も付いていない値は**エラー**です（E033）。値が増えたのは外の変更で、\
                 まだ誰も読んでいないという意味だからです。\n",
                "\nFor an imported enum, a value that appears in no row and carries no `{}` is an **error** (E033): \
                 the value arrived through a change made elsewhere, and nobody has read it yet.\n",
                crate::kw::DEFAULT
            ));
        }
        if f.enums.iter().any(|e| e.default_marks.iter().any(|b| *b)) {
            o.push_str(&tr!("\n`{}` は「この値は専用の行を持たず、既定の行に落ちるのが意図です」という宣言です。付いていない値が\
                        どの行にも名指しされていなければ、`rulec check` が書き忘れとして問います（W111）。\n",
                "\n`{}` declares \"this value has no row of its own and is meant to fall to the default row\". \
                        If a value without the mark is named in no row, `rulec check` asks whether it was \
                        forgotten (W111).\n", crate::kw::DEFAULT));
        }
    }

    // --- Groups. What one word hides.
    if !f.groups.is_empty() {
        o.push_str(&tr!("\n## グループ\n\n", "\n## Groups\n\n"));
        o.push_str(&tr!(
            "グループは列挙の一部に名前を付けたものです。表のセルに書かれた一語が、下の値をまとめて指しています。\n\n",
            "A group is a named subset of an enum. One word written in a table cell stands for all the values below.\n\n"
        ));
        let mut by_enum: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for g in &f.groups {
            let en = c.groups.get(&g.name.text).map(|(e, _)| e.clone()).unwrap_or_default();
            by_enum.entry(en).or_default().push(g.name.text.clone());
            let ms: Vec<String> = g.members.iter().map(|m| m.text.clone()).collect();
            o.push_str(&tr!(
                "- **{}**（{}）— {}\n",
                "- **{}** ({}) — {}\n",
                md_esc(&g.name.text),
                n_values(ms.len()),
                md_esc(&ms.join(sep()))
            ));
        }
        for (en, names) in &by_enum {
            o.push_str(&format!("\n{}\n", group_fact(c, en, names)));
        }
    }

    // --- Constraints. What the table does not have to cover, and why.
    //
    // This is the §1.6 job exactly: a fact the checker used that the table does not show. A
    // reader looking at a `-` cannot tell "this column does not matter here" from "this
    // cannot happen"; the constraints are where the second one is written down, so the
    // approver can agree or disagree with it.
    if !f.constraints.is_empty() {
        o.push_str(&tr!("\n## 起きない組み合わせ\n\n", "\n## Combinations that do not happen\n\n"));
        o.push_str(&tr!(
            "呼び出し側が保証する、入力どうしの関係です。**検査はこれを信じて、満たさない組み合わせには行を要求していません。** 生成コードは、満たさない入力を入口で断ります。\n\n",
            "Relations between inputs that the caller guarantees. **The checks believed them and demanded no row for the combinations they exclude**, and the generated code refuses such an input at the door.\n\n"
        ));
        for k in &f.constraints {
            o.push_str(&format!("- `{} {} {}`\n", md_esc(&k.left), k.op.word(), md_esc(&k.right)));
        }
        o.push('\n');
    }

    // --- What the walk counted. An approver reading the table below sees a column of
    // numbers; this is where those numbers come from, and how long a sequence the rule will
    // take at all (§15.58).
    let counts: Vec<&crate::ast::CountDecl> =
        f.items.iter().filter_map(|i| if let Item::Count(d) = i { Some(d) } else { None }).collect();
    if !counts.is_empty() {
        o.push_str(&tr!("\n## 数え上げ\n\n", "\n## Counts\n\n"));
        o.push_str(&tr!(
            "並びの要素のうち、条件に当てはまるものの数です。表の列に置けます。範囲は宣言されたもので、**並びの長さの上限**でもあります。\n\n",
            "How many elements of the sequence meet one test. A count can be a table column. Its range is the declared one, and it is also **the cap on the sequence**.\n\n"
        ));
        o.push_str(&tr!(
            "| 名前 | 何を数えるか | 範囲 | 注記 |\n|---|---|---|---|\n",
            "| Name | What it counts | Range | Notes |\n|---|---|---|---|\n"
        ));
        for d in &counts {
            let what = match &d.value {
                Some(v) => tr!("{} が {} の要素", "elements whose {} is {}", d.column.text, v.text),
                None => tr!("{} の要素", "elements where {}", d.column.text),
            };
            o.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                md_esc(&d.name.text),
                md_esc(&what),
                md_esc(&range_text(c, &d.name.text)),
                md_esc(&trailing_comment(&lines, d.name.span.line).unwrap_or_default())
            ));
        }
    }

    // --- Derived values and definitions. The invisible axes. What an `apply` brought in is
    // shown under that apply, from the file it came from.
    o.push_str(&defs_section(f, c, &lines, &|n| inlined(f, n)));

    // --- Tables, and the rules applied among them (§15.69), in the order of the file.
    for (k, it) in f.items.iter().enumerate() {
        for a in f.applies.iter().filter(|a| a.at == k) {
            o.push_str(&apply_section(f, c, a, &lines, path));
        }
        let Item::Table(t) = it else { continue };
        if t.applied.is_some() {
            continue;
        }
        o.push_str(&table_section(f, c, t, &lines, path));
    }
    for a in f.applies.iter().filter(|a| a.at >= f.items.len()) {
        o.push_str(&apply_section(f, c, a, &lines, path));
    }

    // --- How an output that several tables define is decided (§15.66)
    for set in c.sets.iter().filter(|s| s.merged()) {
        o.push_str(&set_section(f, c, set, path));
    }

    // --- How the walk ends (§15.56). The arms are the whole of it: one move per verdict,
    // plus the two answers that belong to no element at all.
    if let Some(fold) = &f.fold {
        o.push_str(&tr!(
            "\n## 畳み込み: {} を {} で畳む\n\n要素を順に見て、表が書いた判定ごとに次の手を決めます。**表が完全かつ一意なので、どの要素もちょうど一つの判定に落ちます。**\n\n| 判定 | 手 |\n|---|---|\n",
            "\n## The walk: {} folded on {}\n\nThe elements are taken in order, and the verdict the table wrote for each one decides the next move. **The table is complete and unique, so every element lands on exactly one verdict.**\n\n| Verdict | Move |\n|---|---|\n",
            fold.over,
            fold.verdict
        ));
        for (n, _, sp) in &fold.arms {
            o.push_str(&format!("| {} | `{}` |\n", md_esc(&n.text), md_esc(&arm_src(&lines, sp.line))));
        }
        let answer = |k: &str, e: &Option<crate::ast::Expr>| -> String {
            match e {
                Some(x) => format!("| {k} | `{}` |\n", md_esc(&arm_src(&lines, x.span().line))),
                None => String::new(),
            }
        };
        o.push_str(&answer(crate::kw::EMPTY, &fold.empty));
        o.push_str(&answer(crate::kw::EXHAUSTED, &fold.exhausted));
        o.push_str(&tr!(
            "\n`{}` は要素ゼロ件のときの答え、`{}` は最後まで見終えたときの答えです。この二つは宣言が必須で、`rulec check` が無ければ断ります（E022・E023）。\n",
            "\n`{}` is the answer when the sequence has nothing in it, and `{}` the answer when the walk reached the end. Both must be declared, and `rulec check` refuses a rule that leaves either out (E022, E023).\n",
            crate::kw::EMPTY,
            crate::kw::EXHAUSTED
        ));
    }

    // --- The named sequences the examples walk. Without them an example reads as a word
    // with nothing behind it (§15.56).
    if !f.sequences.is_empty() {
        o.push_str(&tr!("\n## 例がたどる並び\n\n", "\n## The sequences the examples walk\n\n"));
        for sq in &f.sequences {
            o.push_str(&format!("**{}**\n\n", md_esc(&sq.name.text)));
            if sq.rows.is_empty() {
                o.push_str(&tr!("要素ゼロ件。\n\n", "Nothing in it.\n\n"));
                continue;
            }
            let head: Vec<String> = sq.cols.iter().map(|(n, _)| md_esc(n)).collect();
            o.push_str(&format!("| {} |\n|{}|\n", head.join(" | "), vec!["---"; head.len()].join("|")));
            for row in &sq.rows {
                // Verbatim from the source, like every other cell this file renders.
                let cells: Vec<String> =
                    source_cells(&lines, row.span.line).iter().map(|x| md_esc(x)).collect();
                o.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
            o.push('\n');
        }
    }

    // --- Result
    if f.result.is_some() {
        if let Some(r) = &f.result {
            o.push_str(&tr!("\n## 結果\n\n`{}`\n", "\n## Result\n\n`{}`\n", md_esc(&expr_src(&lines, r.span.line))));
        }
    }

    // --- Examples
    if let Some(ex) = &f.examples {
        o.push_str(&tr!("\n## 例（検証済み）\n\n", "\n## Examples (verified)\n\n"));
        if let Some(h) = header_line(&lines, ex.rows.first().map(|r| r.span.line).unwrap_or(1).saturating_sub(1)) {
            // The source spells the output marker `->`; the rendering shows it as `→`, like
            // every other arrow in this document.
            let mut head: Vec<String> = source_cells(&lines, h).into_iter().map(|c| c.replacen("->", "→", 1)).collect();
            let mut rows: Vec<Vec<String>> = ex.rows.iter().map(|r| source_cells(&lines, r.span.line)).collect();
            let row_lines: Vec<usize> = ex.rows.iter().map(|r| r.span.line).collect();
            with_notes(&mut head, &mut rows, &lines, &row_lines);
            o.push_str(&md_table(&head, &rows, false));
        }
        o.push_str(&tr!(
            "\nこの {} 件は `rulec check` が参照評価器で実行し、すべて宣言どおりの値になりました（E107）。例は**実行される仕様**です。\n",
            "\n`rulec check` ran these {} examples through the reference evaluator, and every one produced the declared values (E107). The examples are an **executable specification**.\n",
            ex.rows.len()
        ));
    }

    o
}

/// The move one arm of a `fold` declares, verbatim: what is written after the `->`.
fn arm_src(lines: &[&str], line: usize) -> String {
    let Some(l) = lines.get(line.saturating_sub(1)) else { return String::new() };
    let (body, _) = split_comment(l);
    match body.split_once("->") {
        Some((_, rhs)) => rhs.trim().to_string(),
        None => body.trim().to_string(),
    }
}

/// The right-hand side of `=` on a declaration line, verbatim from the source file.
fn expr_src(lines: &[&str], line: usize) -> String {
    let Some(l) = lines.get(line.saturating_sub(1)) else { return String::new() };
    let (body, _) = split_comment(l);
    let rhs = match body.split_once('=') {
        Some((_, rhs)) => rhs,
        None => body,
    };
    // A derived-value declaration continues as `= expr  range >=… <=…`. The range is shown in
    // its own column, so drop it from the expression — and so is the citation (`@法 第91条`),
    // which is shown in the notes.
    let rhs = match rhs.find('@') {
        Some(i) => &rhs[..i],
        None => rhs,
    };
    match rhs.find(&format!(" {} ", crate::kw::RANGE)) {
        Some(i) => rhs[..i].trim().to_string(),
        None => rhs.trim().to_string(),
    }
}

/// The rows of a table with the trailing comment of each, as a notes column. The column is
/// there only when some row has a comment, so a table without any renders as it always has.
/// A row's comment is where the source it was taken from is written when it differs from the
/// table's (§15.32); it is source text, so showing it keeps every cell traceable.
fn with_notes(head: &mut Vec<String>, rows: &mut [Vec<String>], lines: &[&str], row_lines: &[usize]) {
    let notes: Vec<String> = row_lines.iter().map(|&l| trailing_comment(lines, l).unwrap_or_default()).collect();
    if notes.iter().all(|n| n.is_empty()) {
        return;
    }
    let width = head.len();
    for (r, n) in rows.iter_mut().zip(notes) {
        r.resize(width, String::new());
        r.push(n);
    }
    head.push(tr!("注記", "Notes"));
}

/// `with_notes`, with each row's own citation (§15.68) in the same column, before its comment.
fn with_notes_cites(head: &mut Vec<String>, rows: &mut [Vec<String>], lines: &[&str], row_lines: &[usize], cites: &[String]) {
    let notes: Vec<String> = row_lines
        .iter()
        .enumerate()
        .map(|(i, &l)| {
            let com = trailing_comment(lines, l).unwrap_or_default();
            let cite = cites.get(i).cloned().unwrap_or_default();
            match (cite.is_empty(), com.is_empty()) {
                (true, _) => com,
                (false, true) => tr!("出典: {cite}", "Source: {cite}"),
                (false, false) => tr!("出典: {cite}。{com}", "Source: {cite}. {com}"),
            }
        })
        .collect();
    if notes.iter().all(|n| n.is_empty()) {
        return;
    }
    let width = head.len();
    for (r, n) in rows.iter_mut().zip(notes) {
        r.resize(width, String::new());
        r.push(n);
    }
    head.push(tr!("注記", "Notes"));
}

fn md_table(head: &[String], rows: &[Vec<String>], numbered: bool) -> String {
    let nums: Vec<String> = (1..=rows.len()).map(|i| i.to_string()).collect();
    md_table_nums(head, rows, numbered.then_some(nums.as_slice()))
}

/// A table with a `#` column holding the given names: the row's number as written, or its
/// label when it has one. A label reads as the row's name in the trace and in a later
/// version, so it is what the approver sees too.
fn md_table_nums(head: &[String], rows: &[Vec<String>], nums: Option<&[String]>) -> String {
    let mut o = String::from("|");
    if nums.is_some() {
        o.push_str(" # |");
    }
    for h in head {
        o.push_str(&format!(" {} |", md_esc(h)));
    }
    o.push_str("\n|");
    for _ in 0..head.len() + usize::from(nums.is_some()) {
        o.push_str("---|");
    }
    o.push('\n');
    for (i, r) in rows.iter().enumerate() {
        o.push('|');
        if let Some(ns) = nums {
            o.push_str(&format!(" {} |", md_esc(ns.get(i).map(|s| s.as_str()).unwrap_or(""))));
        }
        for k in 0..head.len() {
            o.push_str(&format!(" {} |", md_esc(r.get(k).map(|s| s.as_str()).unwrap_or(""))));
        }
        o.push('\n');
    }
    o
}

/// The `#` column of a table: each row's label, or its number as written.
fn row_nums(t: &Table) -> Vec<String> {
    t.rows
        .iter()
        .map(|r| match &r.label {
            Some(l) => l.text.clone(),
            None => r.index.to_string(),
        })
        .collect()
}

/// The section of an output that several tables define: which takes precedence over which,
/// whether as an exception (the winner lies inside the loser) or reaching beyond it, the order
/// A note cell: the citation, then the comment.
fn note_with_cite(cite: Option<&Cite>, com: Option<String>) -> String {
    let com = com.unwrap_or_default();
    match (cite, com.is_empty()) {
        (None, _) => com,
        (Some(c), true) => tr!("出典: {}", "Source: {}", cite_text(c)),
        (Some(c), false) => tr!("出典: {}。{com}", "Source: {}. {com}", cite_text(c)),
    }
}

/// A citation as the page names it: `法 別表第一`.
fn cite_text(c: &Cite) -> String {
    format!("{} {}", c.source, c.fragments.join(sep()))
}

/// The citation of a definition: which source, which fragment, and — for a law with a copy
/// beside the rule — the fragment's text, quoted, so that the approver compares the rows
/// with the source on one page (§15.68).
fn cite_section(f: &RuleFile, cite: Option<&Cite>, path: &str, quote: bool) -> String {
    let Some(c) = cite else { return String::new() };
    let decl = f.sources.iter().find(|d| d.name.text == c.source);
    let frags = if c.fragments.is_empty() { String::new() } else { format!(" {}", c.fragments.join(sep())) };
    let line = match decl.map(|d| &d.kind) {
        Some(SourceKind::Law { id, asof }) => {
            // Which text that date reached: the copy's `revision.txt` says which revision
            // e-Gov served — the date it came into force and the amending law (§15.71).
            let base = decl.and_then(|d| d.base.as_deref()).unwrap_or(path);
            let rev = std::fs::read_to_string(crate::sources::copy_dir(base, id, asof).join("revision.txt"))
                .ok()
                .and_then(|r| crate::sources::revision_words(r.trim()))
                .map(|w| tr!("。{w}", "; {w}"))
                .unwrap_or_default();
            tr!("出典: {}{frags}（法令 {id}、{asof} 時点{rev}）", "Source: {}{frags} (law {id}, as of {asof}{rev})", c.source)
        }
        Some(SourceKind::File { path: p, hash }) => {
            let h = hash.as_ref().map(|h| tr!("、sha256:{h}", ", sha256:{h}")).unwrap_or_default();
            tr!("出典: {}{frags}（{p}{h}）", "Source: {}{frags} ({p}{h})", c.source)
        }
        None => tr!("出典: {}{frags}", "Source: {}{frags}", c.source),
    };
    let mut o = format!("{}\n\n", md_esc(&line));
    if quote {
        if let Some(d) = decl {
            for frag in &c.fragments {
                if let Some(text) = crate::sources::fragment_text(path, d, frag) {
                    for l in text.lines() {
                        o.push_str(&format!("> {}\n", md_esc(l)));
                    }
                    o.push('\n');
                }
            }
        }
    }
    o
}

/// The section of an output that several tables or clauses define: which takes precedence
/// over which, whether as an exception (the winner lies inside the loser) or reaching beyond
/// it, the order they are tried in, and what `rulec check` verified over all of them together.
fn set_section(f: &RuleFile, c: &Checked, set: &crate::defset::DefSet, path: &str) -> String {
    let t = &set.table;
    let key = &set.key;
    let mut o = tr!("\n## {key} の決まり方\n\n", "\n## How {key} is decided\n\n");
    let member = |mi: usize| format!("{} {}", set.kind_word(mi), set.members[mi]);
    let rn = |i: usize| -> String {
        let r = &t.rows[i];
        if set.is_clause_row(i) {
            return tr!("節 {}", "clause {}", set.row_table(i));
        }
        match &r.label {
            Some(l) => tr!("表 {} 行{}（{}）", "table {} row {} ({})", set.row_table(i), r.index, l.text),
            None => tr!("表 {} 行{}", "table {} row {}", set.row_table(i), r.index),
        }
    };
    let r = region::check_set(set, c, f, path, region::DEFAULT_BUDGET);
    let all: Vec<String> = (0..set.members.len()).map(member).collect();
    let order: Vec<String> = (0..set.members.len()).rev().map(member).collect();
    o.push_str(&tr!(
        "{} を定めるのは {} で、{} の順に試して、最初に当てはまったものが答えになる。\n\n",
        "{} is defined by {}; they are tried in the order {}, and the first that applies is the answer.\n\n",
        key,
        all.join(sep()),
        order.join(if crate::i18n::ja() { "、" } else { ", " })
    ));
    for e in &set.edges {
        let w = member(e.winner);
        let loser = match e.loser_row {
            Some(j) => rn(j),
            None => member(e.loser),
        };
        let pairs: Vec<&(usize, usize, bool)> = r
            .edge_pairs
            .iter()
            .filter(|(i, j, _)| set.member_of[*i] == e.winner && set.member_of[*j] == e.loser && e.loser_row.is_none_or(|x| x == *j))
            .collect();
        let shape = if pairs.is_empty() {
            tr!("交わる行が無いので、この優先は効いていない（W117）", "no rows meet, so this precedence has no effect (W117)")
        } else if pairs.iter().all(|p| p.2) {
            tr!("交わる {} 対のすべてで、{w} の行は相手の行に収まる（例外）", "in all {} pairs that meet, the rows of {w} lie inside the other's (an exception)", pairs.len())
        } else {
            tr!("交わる {} 対のうち、{w} の行が相手の行の外にも及ぶものがある（優先）", "in some of the {} pairs that meet, the rows of {w} reach beyond the other's (precedence)", pairs.len())
        };
        o.push_str(&tr!("- {w} は {loser} に優先する。{shape}\n", "- {w} takes precedence over {loser}. {shape}\n"));
    }
    o.push_str(&tr!("\n**`rulec check` が確かめたこと**\n\n", "\n**What `rulec check` verified**\n\n"));
    o.push_str(&tr!(
        "- どの入力の組合せも、{} のいずれかの行に当てはまります（E101 完全性）\n",
        "- Every combination of inputs matches some row of {} (E101 completeness)\n",
        all.join(sep())
    ));
    o.push_str(&tr!(
        "- どの入力にも当てはまらない行はありません（E102）\n",
        "- There is no row that can never match (E102 unreachable row)\n"
    ));
    if r.w114.is_empty() {
        o.push_str(&tr!(
            "- 優先の書かれていない重なりはありません（E105）。二つの定義のどちらにも当てはまる入力は、上の優先で決まります\n",
            "- No overlap is left without a precedence (E105). An input that matches two definitions is decided by the precedence above\n"
        ));
    } else {
        let pairs: Vec<String> = r.w114.iter().map(|(i, j)| tr!("{} と {}", "{} and {}", rn(*i), rn(*j))).collect();
        o.push_str(&tr!(
            "- {} が重ならないことは**証明できていません**（W114）。生成コードに実行時のガードが入ります\n",
            "- The exclusivity of {} is **not proven** (W114). The generated code carries a runtime guard\n",
            pairs.join(sep())
        ));
    }
    o
}

/// The text of a clause's `when` or `then` line as written, without the keyword and the
/// trailing comment. It comes from the source line, so what the page shows is what the file
/// says (§1.6).
fn clause_text(lines: &[&str], line: usize, kw: &str) -> String {
    let Some(l) = lines.get(line.saturating_sub(1)) else { return String::new() };
    let (body, _) = split_comment(l);
    let t = body.trim();
    t.strip_prefix(kw).map(|r| r.trim()).unwrap_or(t).to_string()
}

/// The folded header of an output column: the type and the rounding, from the declarations.
fn folded_out_header(f: &RuleFile, c: &Checked, n: &str) -> String {
    let mut bits = vec![match c.ty_of(n) {
        Some(Ty::Enum(e)) => e,
        Some(t) => format!("{t}"),
        None => String::new(),
    }];
    if let Some(rd) = f.outputs.iter().find(|q| q.name.text == *n).and_then(|q| q.rounding.as_ref()) {
        bits.push(format!("{}({})", rd.mode, rd.grid.raw));
    }
    let bits: Vec<String> = bits.into_iter().filter(|b| !b.is_empty()).collect();
    if bits.is_empty() {
        format!("→ {n}")
    } else {
        tr!("→ {n}（{}）", "→ {n} ({})", bits.join(" / "))
    }
}

/// Whether a name is one an `apply` brought in: a callee definition under the apply's
/// prefix, or the definition the expansion made for one of the callee's outputs.
fn inlined(f: &RuleFile, name: &str) -> bool {
    f.applies.iter().any(|a| name.starts_with(&format!("{}:", a.name.text)) || a.defines.iter().any(|d| d == name))
}

/// The `derive`s and `define`s of a rule, as written. `skip` leaves out the ones shown
/// elsewhere.
fn defs_section(f: &RuleFile, c: &Checked, lines: &[&str], skip: &dyn Fn(&str) -> bool) -> String {
    let mut o = String::new();
    let derived: Vec<&DerivedDecl> =
        f.items.iter().filter_map(|i| if let Item::Derived(d) = i { Some(d) } else { None }).filter(|d| !skip(&d.name.text)).collect();
    let defines: Vec<&DefineDecl> =
        f.items.iter().filter_map(|i| if let Item::Define(d) = i { Some(d) } else { None }).filter(|d| !skip(&d.name.text)).collect();
    if !derived.is_empty() || !defines.is_empty() {
        o.push_str(&tr!("\n## 導出と定義\n\n", "\n## Derived Values and Definitions\n\n"));
        o.push_str(&tr!(
            "表の列に置ける中間の値です。式はもとの規則のとおりで、範囲は宣言されたものです。\n\n",
            "Intermediate values that can be placed in a table column. The expressions are as written in the source file, and the ranges are the declared ones.\n\n"
        ));
        o.push_str(&tr!(
            "| 名前 | 種類 | 式 | 範囲 | 注記 |\n|---|---|---|---|---|\n",
            "| Name | Kind | Expression | Range | Notes |\n|---|---|---|---|---|\n"
        ));
        for d in &derived {
            let e = expr_src(&lines, d.name.span.line);
            o.push_str(&tr!(
                "| {} | 導出 | `{}` | {} | {} |\n",
                "| {} | Derived value | `{}` | {} | {} |\n",
                md_esc(&d.name.text),
                md_esc(&e),
                md_esc(&range_text(c, &d.name.text)),
                md_esc(&note_with_cite(d.cite.as_ref(), trailing_comment(&lines, d.name.span.line)))
            ));
        }
        for d in &defines {
            let e = expr_src(&lines, d.name.span.line);
            o.push_str(&tr!(
                "| {} | 定義 | `{}` |  | {} |\n",
                "| {} | Definition | `{}` |  | {} |\n",
                md_esc(&d.name.text),
                md_esc(&e),
                md_esc(&note_with_cite(d.cite.as_ref(), trailing_comment(&lines, d.name.span.line)))
            ));
        }
        if !derived.is_empty() {
            o.push_str(&tr!(
                "\n導出の宣言範囲が、入力範囲から実際に到達しうる値をすべて含んでいることは `rulec check` が確かめました（E112）。\n",
                "\nThat the declared range of every derived value contains all the values actually reachable from the input ranges was verified by `rulec check` (E112).\n"
            ));
        }
    }
    o
}

/// A rule applied by this one (§15.69): what it is, what is bound to what, what is left out,
/// which of its rows this rule never reaches — and then the callee's own definitions, drawn
/// from the callee's file with the callee's sources, one heading level down.
fn apply_section(f: &RuleFile, c: &Checked, a: &ApplyDecl, lines: &[&str], path: &str) -> String {
    let an = &a.name.text;
    let hash = a.hash.as_deref().map(|h| format!("sha256:{h}")).unwrap_or_else(|| tr!("固定なし", "unpinned"));
    let mut o = tr!("\n## 準用 {an}: {}（{hash}）\n\n", "\n## Applied rule {an}: {} ({hash})\n\n", a.path);
    o.push_str(&cite_section(f, a.cite.as_ref(), path, true));
    if let Some(cm) = trailing_comment(lines, a.span.line) {
        o.push_str(&format!("{}\n\n", md_esc(&cm)));
    }
    let callee = crate::prepare(&a.callee_src, &a.callee_path).ok();
    match &callee {
        Some((cf, _)) => {
            o.push_str(&tr!(
                "規則 **{}** v{} を、次のように読み替えて準用する。\n\n",
                "Rule **{}** v{} is applied with the following substitutions.\n\n",
                cf.name.text,
                cf.version
            ));
            if let Some(d) = &cf.description {
                o.push_str(&format!("{}\n\n", md_esc(d)));
            }
        }
        None => o.push_str(&tr!("元の規則 `{}` は読めなかった。\n\n", "The callee `{}` could not be read.\n\n", a.path)),
    }
    o.push_str(&tr!(
        "| 元の規則の入力 | この規則の値 | 値の対応 |\n|---|---|---|\n",
        "| Callee input | What this rule passes | Value mapping |\n|---|---|---|\n"
    ));
    for b in &a.bindings {
        let v = match &b.value {
            BindValue::Name(n) => n.clone(),
            BindValue::Lit(l) => lit_text(l),
        };
        let m: Vec<String> = b.map.iter().map(|(from, to)| format!("{from} → {to}")).collect();
        o.push_str(&format!("| {} | {} | {} |\n", md_esc(&b.input), md_esc(&v), md_esc(&m.join(sep()))));
    }
    if !a.excepts.is_empty() {
        let kinds: Vec<String> = a
            .excepts
            .iter()
            .map(|(t, _)| {
                let kind = callee.as_ref().and_then(|(cf, _)| {
                    cf.items.iter().find_map(|it| match it {
                        Item::Table(x) if x.name.as_ref().is_some_and(|n| n.text == *t) => {
                            Some(if x.clause { tr!("節 {t}", "clause {t}") } else { tr!("表 {t}", "table {t}") })
                        }
                        _ => None,
                    })
                });
                kind.unwrap_or_else(|| tr!("行 {t}", "row {t}"))
            })
            .collect();
        o.push_str(&tr!("\n準用しない定義: {}。\n", "\nLeft out: {}.\n", kinds.join(sep())));
    }
    if !a.outputs.is_empty() {
        o.push_str(&tr!(
            "\n| 元の規則の出力 | この規則での名前 |\n|---|---|\n",
            "\n| Callee output | Its name in this rule |\n|---|---|\n"
        ));
        for ob in &a.outputs {
            o.push_str(&format!("| {} | {} |\n", md_esc(&ob.output), md_esc(&ob.name.text)));
        }
    }
    // The callee's rows this rule never reaches: what it binds is narrower than what the
    // callee was written for. They are listed, not reported (§5.3 of the draft).
    let prefix = format!("{an}:");
    let mut unused: Vec<String> = Vec::new();
    for (si, chk) in crate::table_checks(f, c, path).iter().enumerate() {
        let set = &c.sets[si];
        if !set.applied.iter().any(|x| x.as_deref() == Some(an.as_str())) {
            continue;
        }
        let mut by_table: Vec<(String, Vec<String>)> = Vec::new();
        for &i in &chk.dead {
            let tn = set.row_table(i).to_string();
            if !tn.starts_with(&prefix) {
                continue;
            }
            let r = &set.table.rows[i];
            let rn = match &r.label {
                Some(l) => l.text.clone(),
                None => tr!("行{}", "row {}", r.index),
            };
            match by_table.iter_mut().find(|(t, _)| *t == tn) {
                Some((_, rs)) => rs.push(rn),
                None => by_table.push((tn, vec![rn])),
            }
        }
        for (tn, rs) in by_table {
            let shown = tn.strip_prefix(&prefix).unwrap_or(&tn).to_string();
            unused.push(tr!("表 {shown}: {}", "table {shown}: {}", rs.join(sep())));
        }
    }
    if !unused.is_empty() {
        o.push_str(&tr!(
            "\nこの準用では当たらない行（この規則の入力の範囲では届かない）: {}。\n",
            "\nRows this apply never uses (unreachable from this rule's ranges): {}.\n",
            unused.join(if crate::i18n::ja() { "；" } else { "; " })
        ));
    }
    // The callee, as written, under this apply.
    if let Some((cf, cc)) = &callee {
        let clines: Vec<&str> = a.callee_src.lines().collect();
        let mut inner = String::new();
        inner.push_str(&defs_section(cf, cc, &clines, &|_| false));
        for it in &cf.items {
            let Item::Table(t) = it else { continue };
            let tn = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
            if a.excepts.iter().any(|(e, _)| *e == tn) {
                continue;
            }
            inner.push_str(&table_section(cf, cc, t, &clines, &a.callee_path));
        }
        // A set one of whose members is left out is not the set this rule runs; the page
        // says how this rule decides the output in its own section.
        for set in cc.sets.iter().filter(|s| s.merged() && !s.members.iter().any(|m| a.excepts.iter().any(|(e, _)| e == m))) {
            inner.push_str(&set_section(cf, cc, set, &a.callee_path));
        }
        // One heading level down: the callee's sections sit under this apply.
        o.push_str(&inner.replace("\n## ", "\n### ").replace("\n**`rulec check` が確かめたこと**", "\n**元の規則の `rulec check` が確かめたこと**").replace("\n**What `rulec check` verified**", "\n**What `rulec check` verified of the callee**"));
    }
    o
}

fn lit_text(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Word(w) => w.clone(),
        Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        Lit::Str(s) => format!("\"{s}\""),
    }
}

/// A clause on the approver's page: the condition and the value as written, and what it
/// takes precedence over. A clause is one row, so the page shows one row.
fn clause_section(f: &RuleFile, c: &Checked, t: &Table, lines: &[&str], _path: &str) -> String {
    let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    let out = t.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    let mut o = tr!("\n## 節 {name} → {out}\n\n", "\n## Clause {name} → {out}\n\n");
    o.push_str(&cite_section(f, t.cite.as_ref(), _path, true));
    if let Some(cm) = trailing_comment(lines, t.span.line) {
        o.push_str(&format!("{}\n\n", md_esc(&cm)));
    }
    o.push_str(&tr!("| 列 | 出どころ |\n|---|---|\n", "| Column | Source |\n|---|---|\n"));
    for (col, _) in &t.inputs {
        o.push_str(&format!("| {} | {} |\n", md_esc(col), md_esc(&producer(f, col))));
    }
    let dest = if f.outputs.iter().any(|q| q.name.text == out) {
        tr!("この規則の出力", "Output of this rule")
    } else {
        tr!("後の表か節の列", "A column of a later table or clause")
    };
    o.push_str(&format!("| → {} | {} |\n", md_esc(&out), md_esc(&dest)));
    let Some(row) = t.rows.first() else { return o };
    let when = clause_text(lines, row.span.line, crate::kw::WHEN);
    let then = clause_text(lines, row.out_spans.first().map(|s| s.line).unwrap_or(row.span.line), crate::kw::THEN);
    let head = vec![tr!("条件", "Condition"), folded_out_header(f, c, &out)];
    o.push('\n');
    o.push_str(&md_table_nums(&head, &[vec![when, then]], Some(&["1".to_string()])));
    if !t.overrides.is_empty() {
        let ts: Vec<String> = t
            .overrides
            .iter()
            .map(|r| match &r.row {
                Some(l) => tr!("表 {} 行 {l}", "table {} row {l}", r.table),
                None => r.table.clone(),
            })
            .collect();
        o.push_str(&tr!("\nこの節は {} に優先する。\n", "\nThis clause takes precedence over {}.\n", ts.join(sep())));
    }
    if let Some(set) = c.set_of_table(t).filter(|s| s.merged()) {
        o.push_str(&tr!(
            "\n検査の結果は「{} の決まり方」の節にある。\n",
            "\nWhat `rulec check` verified is under \"How {} is decided\".\n",
            set.key
        ));
        return o;
    }
    o.push_str(&tr!("\n**`rulec check` が確かめたこと**\n\n", "\n**What `rulec check` verified**\n\n"));
    o.push_str(&tr!(
        "- どの入力の組合せも、この節に当てはまります（E101 完全性）\n",
        "- Every combination of inputs matches this clause (E101 completeness)\n"
    ));
    o.push_str(&tr!(
        "- この節に当てはまる入力があります（E102）\n",
        "- Some input reaches this clause (E102 unreachable row)\n"
    ));
    o
}

fn table_section(f: &RuleFile, c: &Checked, t: &Table, lines: &[&str], path: &str) -> String {
    if t.clause {
        return clause_section(f, c, t, lines, path);
    }
    let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
    let mut o = tr!("\n## 表 {name}（{} {policy}）\n\n", "\n## Table {name} ({} {policy})\n\n", crate::kw::POLICY);
    o.push_str(&cite_section(f, t.cite.as_ref(), path, true));

    // The comment at the end of the `table` line is where the source of the table is written
    // (AGENTS.md §1). It goes right under the heading, before anything this rendering adds.
    if let Some(cm) = trailing_comment(lines, t.span.line) {
        o.push_str(&format!("{}\n\n", md_esc(&cm)));
    }

    // Where each value comes from, and where it flows to.
    o.push_str(&tr!("| 列 | 出どころ |\n|---|---|\n", "| Column | Source |\n|---|---|\n"));
    for (col, _) in &t.inputs {
        o.push_str(&format!("| {} | {} |\n", md_esc(col), md_esc(&producer(f, col))));
    }
    for oc in &t.outputs {
        let n = &oc.name.text;
        let uses: Vec<String> = f
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Table(x) if x.inputs.iter().any(|(cc, _)| cc == n) => {
                    Some(tr!("表 {}", "table {}", x.name.as_ref().map(|q| q.text.clone()).unwrap_or_default()))
                }
                _ => None,
            })
            .collect();
        let dest = if f.outputs.iter().any(|q| q.name.text == *n) {
            tr!("この規則の出力", "Output of this rule")
        } else if uses.is_empty() {
            tr!("（この表の中だけ）", "(this table only)")
        } else {
            tr!("→ {} の入力列", "→ input column of {}", uses.join(sep()))
        };
        o.push_str(&format!("| → {} | {} |\n", md_esc(n), md_esc(&dest)));
    }

    let head_l = header_line(lines, t.span.line).unwrap_or(t.span.line);
    let mut head = source_cells(lines, head_l);
    // An output column's header is long if left as the source's type annotation
    // (`→ サイズ(size) : サイズ区分`). Fold the unit, tax kind and rounding grid into a form the
    // approver reads. Everything folded in comes from the declarations, so it traces back to
    // the source file.
    for (k, oc) in t.outputs.iter().enumerate() {
        let i = t.inputs.len() + k;
        if i >= head.len() {
            break;
        }
        let n = &oc.name.text;
        let mut bits = vec![match c.ty_of(n) {
            Some(Ty::Enum(e)) => e,
            Some(t) => format!("{t}"),
            None => String::new(),
        }];
        if let Some(rd) = f.outputs.iter().find(|q| q.name.text == *n).and_then(|q| q.rounding.as_ref()) {
            bits.push(format!("{}({})", rd.mode, rd.grid.raw));
        }
        let bits: Vec<String> = bits.into_iter().filter(|b| !b.is_empty()).collect();
        head[i] = if bits.is_empty() {
            format!("→ {n}")
        } else {
            tr!("→ {n}（{}）", "→ {n} ({})", bits.join(" / "))
        };
    }
    let mut rows: Vec<Vec<String>> = t.rows.iter().map(|r| source_cells(lines, r.span.line)).collect();
    let row_lines: Vec<usize> = t.rows.iter().map(|r| r.span.line).collect();
    let cites: Vec<String> = t.rows.iter().map(|r| r.cite.as_ref().map(cite_text).unwrap_or_default()).collect();
    with_notes_cites(&mut head, &mut rows, lines, &row_lines, &cites);
    o.push('\n');
    o.push_str(&md_table_nums(&head, &rows, Some(&row_nums(t))));

    // The default row. Under `上から` (`first`), a last all-`-` row means "everything else gets
    // this".
    let default_row = t
        .rows
        .iter()
        .position(|r| !r.cells.is_empty() && r.cells.iter().all(|x| matches!(x, Cell::DontCare)));
    if let Some(i) = default_row {
        o.push_str(&tr!(
            "\n行{} はすべての列が `-` なので、上のどれにも当てはまらない入力が落ちる**既定行**です。\n",
            "\nRow {} has `-` in every column, so it is the **default row** where inputs that match none of the rows above fall.\n",
            i + 1
        ));
    }

    // If groups appear in the cells, say which ones to look at.
    // Whether a group appeared in a cell, and whether it appeared in the `以外:` (`not:`) form.
    // The size of the complement is added only for the latter (showing the scale of a form
    // that is not used would be noise).
    let mut used: BTreeMap<String, bool> = BTreeMap::new();
    for cell in t.rows.iter().flat_map(|r| r.cells.iter()) {
        let (words, negated): (Vec<&Lit>, bool) = match cell {
            Cell::Lit(l) => (vec![l], false),
            Cell::Set(ls) => (ls.iter().collect(), false),
            Cell::Not(ls) => (ls.iter().collect(), true),
            _ => (vec![], false),
        };
        for l in words {
            let Lit::Word(w) = l else { continue };
            if c.groups.contains_key(w) {
                let e = used.entry(w.clone()).or_insert(false);
                *e |= negated;
            }
        }
    }
    if !used.is_empty() {
        // §1.6 condition: `以外: 群` (`not: group`) is not expanded, but **the count is added**.
        // Listing 41 prefectures does not survive a visual check, so not expanding is right,
        // but "it is the complement" alone cannot answer the approver's first question (is the
        // scale plausible?). The cardinality can be counted from the declarations.
        let n = |g: &str| -> Option<(usize, usize)> {
            let (en, ms) = c.groups.get(g)?;
            Some((ms.len(), c.enums.get(en)?.len()))
        };
        let list: Vec<String> = used
            .iter()
            .map(|(g, negated)| match (n(g), negated) {
                (Some((k, all)), true) => {
                    tr!(
                        "**{g}**（{}、`{}: {g}` は残り {}）",
                        "**{g}** ({}; `{}: {g}` is the remaining {})",
                        n_values(k),
                        crate::kw::NOT,
                        n_values(all - k)
                    )
                }
                (Some((k, _)), false) => tr!("**{g}**（{}）", "**{g}** ({})", n_values(k)),
                _ => format!("**{g}**"),
            })
            .collect();
        o.push_str(&tr!(
            "\nこの表のセルに現れるグループ: {}。中身は「グループ」の節にあります。\n",
            "\nGroups appearing in the cells of this table: {}. Their members are in the \"Groups\" section.\n",
            list.join(sep())
        ));
    }

    // A table that shares its output with others is verified as a set; the facts are under
    // the set's own section (§15.66).
    if let Some(set) = c.set_of_table(t).filter(|s| s.merged()) {
        o.push_str(&tr!(
            "\n検査の結果は「{} の決まり方」の節にある。\n",
            "\nWhat `rulec check` verified is under \"How {} is decided\".\n",
            set.key
        ));
        return o;
    }

    // Verified statements. This part alone writes nothing but what `rulec check` verified.
    o.push_str(&tr!("\n**`rulec check` が確かめたこと**\n\n", "\n**What `rulec check` verified**\n\n"));
    o.push_str(&tr!(
        "- どの入力の組合せも、いずれかの行に当てはまります（E101 完全性）\n",
        "- Every combination of inputs matches some row (E101 completeness)\n"
    ));
    o.push_str(&tr!(
        "- どの入力にも当てはまらない行はありません（E102）\n",
        "- There is no row that can never match (E102 unreachable row)\n"
    ));
    let r = region::check_table(t, c, f, path, region::DEFAULT_BUDGET);
    match t.policy {
        Policy::Unique => {
            if r.w114.is_empty() {
                o.push_str(&tr!(
                    "- 二つ以上の行に同時に当てはまる入力はありません（E105 重なり）。行の並べ替えは意味を変えません\n",
                    "- No input matches two or more rows at once (E105 overlap). Reordering the rows does not change the meaning\n"
                ));
            } else {
                let pairs: Vec<String> =
                    r.w114.iter().map(|(i, j)| tr!("行{} と 行{}", "row {} and row {}", i + 1, j + 1)).collect();
                o.push_str(&tr!(
                    "- 行の重なりは見つかりませんでしたが、{} が重ならないことは**証明できていません**（W114）。\
                     万一その入力が来たとき黙って先の行を選ばないよう、生成コードに実行時のガードが入ります\n",
                    "- No overlapping rows were found, but the exclusivity of {} is **not proven** (W114). \
                     So that such an input, should it ever arrive, is not silently given the earlier row, \
                     the generated code carries a runtime guard\n",
                    pairs.join(sep())
                ));
            }
        }
        Policy::TopDown => {
            let s = r.shadow;
            if s.total() > 0 {
                o.push_str(&tr!(
                    "- 行の重なりは {} 対あります（先に書いた行が後ろを隠すだけの**階段** {}、\
                     どちらが勝っても値の変わらない**同じ答え** {}、出力が食い違うので**要確認** {}）。\
                     `{} {}` なので、先に書かれた行が勝ちます\n",
                    "- There are {} pairs of overlapping rows (**structural** {}, the normal shape of a \
                     staircase; **equivalent** {}, where the value is the same whichever row wins; \
                     **needs review** {}, where the outputs differ). Because of `{} {}`, the row written \
                     first wins\n",
                    s.total(),
                    s.structural,
                    s.equivalent,
                    s.confirm,
                    crate::kw::POLICY,
                    crate::kw::FIRST
                ));
                // Only "needs review" pairs get the row pair and the witness. This is the only
                // place where the approver can answer "is this intended?"; a count alone gives
                // them nothing to answer.
                for d in r.diags.iter().filter(|d| d.code == "W105") {
                    let w = d
                        .notes
                        .iter()
                        .find(|n| n.starts_with(&tr!("両方に当てはまる例:", "Both rows match:")))
                        .cloned()
                        .unwrap_or_default();
                    o.push_str(&tr!(
                        "  - 要確認: {}。{}\n",
                        "  - Needs review: {}. {}\n",
                        md_esc(&d.title.replacen(&tr!("行の重なり: ", "Overlapping rows: "), "", 1)),
                        md_esc(&w)
                    ));
                }
            } else {
                o.push_str(&tr!(
                    "- 行の重なりはありません。`{} {}` にすれば、並べ替えが意味を変えないことを検査が保証できます（W110）\n",
                    "- There are no overlapping rows. With `{} {}`, the checker could guarantee that reordering does not change the meaning (W110)\n",
                    crate::kw::POLICY,
                    crate::kw::UNIQUE
                ));
            }
        }
    }
    o
}

// ---------------------------------------------------------------------------
// The rendering for the customer (§15.61).
// ---------------------------------------------------------------------------

/// The same rule as the article a help centre publishes: what decides the amount, the tables
/// in the words of the business, the cases on either side of every threshold (the vector
/// suite already found them), and the worked examples. Nothing is written here that the
/// approver's rendering did not derive from the file; what is left out is what a customer is
/// not asked to trust — aliases, the ranges the proof quantified over, the diagnostic codes.
pub fn render_customer(f: &RuleFile, c: &Checked, src: &str, path: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let hash = crate::codegen::hash(src);
    let mut o = String::new();
    o.push_str(&tr!(
        "<!-- rulec {} が {path} (sha256:{}) から生成した案内。本物は .rule のほうで、ここを編集しても戻せません。 -->\n",
        "<!-- Generated by rulec {} from {path} (sha256:{}). The source of truth is the .rule file; edits here cannot be carried back. -->\n",
        env!("CARGO_PKG_VERSION"),
        &hash[..12]
    ));
    o.push_str(&format!("# {}\n", md_esc(&f.name.text)));
    if let Some(d) = &f.description {
        o.push_str(&format!("\n{d}\n"));
    }
    o.push_str(&tr!("\n版 v{}\n", "\nVersion v{}\n", f.version));

    // --- What the answer is, and what it is decided from.
    let outs: Vec<String> = f.outputs.iter().map(|od| md_esc(&od.name.text)).collect();
    o.push_str(&tr!("\n## {}の決まり方\n\n", "\n## What decides {}\n\n", outs.join(sep())));
    for i in &f.inputs {
        o.push_str(&format!("- **{}**: {}\n", md_esc(&i.name.text), md_esc(&customer_input(c, &i.name.text))));
    }
    if let Some(el) = &f.elements {
        o.push_str(&tr!(
            "- **{}**: 一件ずつ順に見ていく並び。各件には {} があります\n",
            "- **{}**: a sequence, taken one element at a time. Each element carries {}\n",
            md_esc(&el.name.text),
            el.fields.iter().map(|fd| md_esc(&fd.name.text)).collect::<Vec<_>>().join(sep())
        ));
    }
    for od in &f.outputs {
        let words = customer_output(c, &od.name.text);
        o.push_str(&match &od.rounding {
            Some(r) => tr!("\n**{}**は{}で、端数は {}。\n", "\n**{}** is {}, {}.\n", md_esc(&od.name.text), words, rounding_verb(r)),
            None => tr!("\n**{}**は{}です。\n", "\n**{}** is {}.\n", md_esc(&od.name.text), words),
        });
    }

    // --- Groups: what one word in a table stands for.
    if !f.groups.is_empty() {
        o.push_str(&tr!("\n## 区分\n\n", "\n## Groupings\n\n"));
        for g in &f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| m.text.clone()).collect();
            o.push_str(&format!("- **{}**: {}\n", md_esc(&g.name.text), md_esc(&ms.join(sep()))));
        }
    }

    // --- Constraints: what the caller promises, in one line each.
    if !f.constraints.is_empty() {
        o.push_str(&tr!("\n## 前提\n\n", "\n## Assumptions\n\n"));
        for k in &f.constraints {
            o.push_str(&format!("- `{} {} {}`\n", md_esc(&k.left), k.op.word(), md_esc(&k.right)));
        }
    }

    // --- Values computed along the way, as written.
    let derived: Vec<&DerivedDecl> =
        f.items.iter().filter_map(|i| if let Item::Derived(d) = i { Some(d) } else { None }).filter(|d| !inlined(f, &d.name.text)).collect();
    let defines: Vec<&DefineDecl> =
        f.items.iter().filter_map(|i| if let Item::Define(d) = i { Some(d) } else { None }).filter(|d| !inlined(f, &d.name.text)).collect();
    let counts: Vec<&crate::ast::CountDecl> =
        f.items.iter().filter_map(|i| if let Item::Count(d) = i { Some(d) } else { None }).collect();
    if !derived.is_empty() || !defines.is_empty() || !counts.is_empty() {
        o.push_str(&tr!("\n## 計算の途中で使う値\n\n", "\n## Values used along the way\n\n"));
        for d in &counts {
            let what = match &d.value {
                Some(v) => tr!("{} が {} の件数", "the number of elements whose {} is {}", d.column.text, v.text),
                None => tr!("{} の件数", "the number of elements where {}", d.column.text),
            };
            o.push_str(&format!("- **{}**: {}\n", md_esc(&d.name.text), md_esc(&what)));
        }
        for d in &derived {
            o.push_str(&format!("- **{}** = `{}`\n", md_esc(&d.name.text), md_esc(&expr_src(&lines, d.name.span.line))));
        }
        for d in &defines {
            o.push_str(&format!("- **{}** = `{}`\n", md_esc(&d.name.text), md_esc(&expr_src(&lines, d.name.span.line))));
        }
    }

    // --- Tables, and the rules applied among them.
    for (k, it) in f.items.iter().enumerate() {
        for a in f.applies.iter().filter(|a| a.at == k) {
            o.push_str(&customer_apply(a, &lines));
        }
        let Item::Table(t) = it else { continue };
        if t.applied.is_some() {
            continue;
        }
        o.push_str(&customer_table(f, c, t, &lines));
    }
    for a in f.applies.iter().filter(|a| a.at >= f.items.len()) {
        o.push_str(&customer_apply(a, &lines));
    }
    for set in c.sets.iter().filter(|s| s.merged()) {
        let order: Vec<String> = set.members.iter().rev().map(|m| tr!("「{m}」", "\"{m}\"")).collect();
        o.push_str(&tr!(
            "\n{} は、{} の順に表を見て、最初に当てはまった行で決まります。\n",
            "\n{} is decided by the first row that applies, looking at the tables in this order: {}.\n",
            set.key,
            order.join(sep())
        ));
    }

    // --- How the walk ends, when there is one.
    if let Some(fold) = &f.fold {
        o.push_str(&tr!(
            "\n## {} の見方\n\n一件ずつ順に見て、判定ごとに次のように進みます。\n\n| 判定 | 進み方 |\n|---|---|\n",
            "\n## Reading {}\n\nThe elements are taken in order, and each verdict decides the next move.\n\n| Verdict | Move |\n|---|---|\n",
            fold.over
        ));
        for (n, _, sp) in &fold.arms {
            o.push_str(&format!("| {} | `{}` |\n", md_esc(&n.text), md_esc(&arm_src(&lines, sp.line))));
        }
        for (k, e) in [(crate::kw::EMPTY, &fold.empty), (crate::kw::EXHAUSTED, &fold.exhausted)] {
            if let Some(x) = e {
                o.push_str(&format!("| {k} | `{}` |\n", md_esc(&arm_src(&lines, x.span().line))));
            }
        }
    }

    // --- The final answer, when it is a formula.
    if let Some(r) = &f.result {
        o.push_str(&tr!("\n## 最後に行う計算\n\n`{}`\n", "\n## The final step\n\n`{}`\n", md_esc(&expr_src(&lines, r.span.line))));
    }

    // --- Either side of every threshold. These are the vectors the boundary-pair criterion
    // built (§9.2): the same input moved one step across a boundary, everything else held.
    let vs = crate::vectors::generate(f, c);
    let (sfx_in, sfx_out) = (tr!("内側", " inside"), tr!("外側", " outside"));
    let mut pairs: Vec<(String, Option<&crate::vectors::Vector>, Option<&crate::vectors::Vector>)> = Vec::new();
    for v in &vs {
        let (key, inside) = match (v.why.strip_suffix(sfx_in.as_str()), v.why.strip_suffix(sfx_out.as_str())) {
            (Some(k), _) => (k.to_string(), true),
            (None, Some(k)) => (k.to_string(), false),
            _ => continue,
        };
        let slot = match pairs.iter().position(|(k, _, _)| *k == key) {
            Some(i) => &mut pairs[i],
            None => {
                pairs.push((key, None, None));
                pairs.last_mut().unwrap()
            }
        };
        if inside {
            slot.1 = Some(v);
        } else {
            slot.2 = Some(v);
        }
    }
    let pairs: Vec<(&crate::vectors::Vector, &crate::vectors::Vector)> =
        pairs.iter().filter_map(|(_, a, b)| Some((a.as_ref()?, b.as_ref()?))).map(|(a, b)| (*a, *b)).collect();
    if !pairs.is_empty() {
        o.push_str(&tr!(
            "\n## 境目の例\n\n条件の境目をまたぐと、答えがどう変わるかの例です。\n\n",
            "\n## At the thresholds\n\nHow the answer changes on either side of a threshold.\n\n"
        ));
        for (a, b) in pairs {
            let Some(col) = a.input.keys().find(|k| a.input.get(*k) != b.input.get(*k)) else { continue };
            let fixed: Vec<String> = a
                .input
                .iter()
                .filter(|(k, _)| *k != col)
                .map(|(k, v)| tr!("{k}: {}", "{k} {}", customer_val(c, k, v)))
                .collect();
            let outs = |v: &crate::vectors::Vector| -> String {
                v.outputs
                    .iter()
                    .map(|(n, x)| match x {
                        Some(x) => format!("{n} {}", customer_val(c, n, x)),
                        None => tr!("{n} は決まらない", "{n} undefined"),
                    })
                    .collect::<Vec<_>>()
                    .join(if crate::i18n::ja() { "・" } else { ", " })
            };
            let held = if fixed.is_empty() { String::new() } else { tr!("（{}）", " ({})", fixed.join(sep())) };
            o.push_str(&tr!(
                "- {col}が {} のとき {}、{} のとき {}{}\n",
                "- {col} {} → {}; {} → {}{}\n",
                customer_val(c, col, &a.input[col]),
                outs(a),
                customer_val(c, col, &b.input[col]),
                outs(b),
                held
            ));
        }
    }

    // --- The worked examples, as written.
    if let Some(ex) = &f.examples {
        o.push_str(&tr!("\n## 例\n\n", "\n## Examples\n\n"));
        if let Some(h) = header_line(&lines, ex.rows.first().map(|r| r.span.line).unwrap_or(1).saturating_sub(1)) {
            let head: Vec<String> =
                source_cells(&lines, h).into_iter().map(|c| strip_alias(&c.replacen("->", "→", 1))).collect();
            let rows: Vec<Vec<String>> = ex.rows.iter().map(|r| source_cells(&lines, r.span.line)).collect();
            o.push_str(&md_table(&head, &rows, false));
        }
    }
    o
}

/// One table, for the customer: the source cells in plainer words, the column headings without
/// A rule applied by this one, for the customer: whose rule, read how, with what left out —
/// and that rule's own tables under it.
fn customer_apply(a: &ApplyDecl, lines: &[&str]) -> String {
    let mut o = tr!("\n## {}（{} の決まりを使う）\n\n", "\n## {} (using the rule in {})\n\n", a.name.text, a.path);
    if let Some(cm) = trailing_comment(lines, a.span.line) {
        o.push_str(&format!("{}\n\n", md_esc(&cm)));
    }
    let callee = crate::prepare(&a.callee_src, &a.callee_path).ok();
    if let Some((cf, _)) = &callee {
        o.push_str(&tr!("「{}」の決まりを、次のように読み替えて使います。\n\n", "The rule \"{}\" is used, read as follows.\n\n", cf.name.text));
    }
    for b in &a.bindings {
        let v = match &b.value {
            BindValue::Name(n) => n.clone(),
            BindValue::Lit(l) => lit_text(l),
        };
        let m: Vec<String> = b.map.iter().map(|(from, to)| tr!("{from} は {to} として", "{from} as {to}")).collect();
        if m.is_empty() {
            o.push_str(&tr!("- {} は {} のこと\n", "- {} means {}\n", md_esc(&b.input), md_esc(&v)));
        } else {
            o.push_str(&tr!("- {} は {} のこと（{}）\n", "- {} means {} ({})\n", md_esc(&b.input), md_esc(&v), m.join(sep())));
        }
    }
    for (t, _) in &a.excepts {
        o.push_str(&tr!("- 「{t}」は使いません\n", "- \"{t}\" is not used\n"));
    }
    for ob in &a.outputs {
        o.push_str(&tr!("- {} を {} とします\n", "- {} becomes {}\n", md_esc(&ob.output), md_esc(&ob.name.text)));
    }
    if let Some((cf, cc)) = &callee {
        let clines: Vec<&str> = a.callee_src.lines().collect();
        let mut inner = String::new();
        for it in &cf.items {
            let Item::Table(t) = it else { continue };
            let tn = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
            if a.excepts.iter().any(|(e, _)| *e == tn) {
                continue;
            }
            inner.push_str(&customer_table(cf, cc, t, &clines));
        }
        o.push_str(&inner.replace("\n## ", "\n### "));
    }
    o
}

/// A clause for the customer: the condition and the value as written, in one row.
fn customer_clause(f: &RuleFile, c: &Checked, t: &Table, lines: &[&str]) -> String {
    let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    let out = t.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    let mut o = format!("\n## {name}\n\n");
    o.push_str(&cite_section(f, t.cite.as_ref(), "", false));
    if let Some(cm) = trailing_comment(lines, t.span.line) {
        o.push_str(&format!("{}\n\n", md_esc(&cm)));
    }
    let Some(row) = t.rows.first() else { return o };
    let when = clause_text(lines, row.span.line, crate::kw::WHEN);
    let then = clause_text(lines, row.out_spans.first().map(|s| s.line).unwrap_or(row.span.line), crate::kw::THEN);
    let mut bits: Vec<String> = Vec::new();
    if let Some(Ty::Money { tax: Some(tx), .. }) = c.ty_of(&out) {
        bits.push(tax_words(&tx));
    }
    if let Some(rd) = f.outputs.iter().find(|q| q.name.text == out).and_then(|q| q.rounding.as_ref()) {
        bits.push(rounding_words(rd));
    }
    let oh = if bits.is_empty() { format!("→ {out}") } else { tr!("→ {out}（{}）", "→ {out} ({})", bits.join(sep())) };
    let head = vec![tr!("条件", "Condition"), oh];
    o.push_str(&md_table_nums(&head, &[vec![when, then]], Some(&["1".to_string()])));
    if !t.overrides.is_empty() {
        let ts: Vec<String> = t.overrides.iter().map(|r| format!("「{}」", r.table)).collect();
        o.push_str(&tr!("\nこの決まりは {} より優先します。\n", "\nThis rule takes precedence over {}.\n", ts.join(sep())));
    }
    o
}

/// aliases and types, and the groups a cell names spelled out underneath.
fn customer_table(f: &RuleFile, c: &Checked, t: &Table, lines: &[&str]) -> String {
    if t.clause {
        return customer_clause(f, c, t, lines);
    }
    let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    let mut o = format!("\n## {}\n\n", md_esc(&name));
    if let Some(cm) = trailing_comment(lines, t.span.line) {
        o.push_str(&format!("{}\n\n", md_esc(&cm)));
    }
    let head_l = header_line(lines, t.span.line).unwrap_or(t.span.line);
    let mut head = source_cells(lines, head_l);
    for h in head.iter_mut().take(t.inputs.len()) {
        *h = strip_alias(h);
    }
    for (k, oc) in t.outputs.iter().enumerate() {
        let i = t.inputs.len() + k;
        if i >= head.len() {
            break;
        }
        let n = &oc.name.text;
        let mut bits: Vec<String> = Vec::new();
        if let Some(Ty::Money { tax: Some(tx), .. }) = c.ty_of(n) {
            bits.push(tax_words(&tx));
        }
        if let Some(rd) = f.outputs.iter().find(|q| q.name.text == *n).and_then(|q| q.rounding.as_ref()) {
            bits.push(rounding_words(rd));
        }
        head[i] = if bits.is_empty() { format!("→ {n}") } else { tr!("→ {n}（{}）", "→ {n} ({})", bits.join(sep())) };
    }
    let rows: Vec<Vec<String>> = t
        .rows
        .iter()
        .map(|r| source_cells(lines, r.span.line).iter().map(|x| customer_cell(x)).collect())
        .collect();
    o.push_str(&md_table_nums(&head, &rows, Some(&row_nums(t))));
    if t.policy == Policy::TopDown {
        o.push_str(&tr!(
            "\n上から順に見て、最初に当てはまる行が答えです。\n",
            "\nThe rows are read from the top, and the first row that matches is the answer.\n"
        ));
    }
    if let Some(i) = t.rows.iter().position(|r| !r.cells.is_empty() && r.cells.iter().all(|x| matches!(x, Cell::DontCare))) {
        o.push_str(&tr!(
            "\n行{} は、上のどれにも当てはまらない場合の答えです。\n",
            "\nRow {} is the answer when none of the rows above applies.\n",
            i + 1
        ));
    }
    let mut used: Vec<String> = Vec::new();
    for cell in t.rows.iter().flat_map(|r| r.cells.iter()) {
        let words: Vec<&Lit> = match cell {
            Cell::Lit(l) => vec![l],
            Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
            _ => vec![],
        };
        for l in words {
            let Lit::Word(w) = l else { continue };
            if c.groups.contains_key(w) && !used.contains(w) {
                used.push(w.clone());
            }
        }
    }
    for g in used {
        if let Some((_, ms)) = c.groups.get(&g) {
            o.push_str(&tr!("\n「{g}」は {} を指します。\n", "\n\"{g}\" stands for {}.\n", md_esc(&ms.join(sep()))));
        }
    }
    o
}

/// `届け先(dest)` → `届け先`: the alias is for the generated code, not for the reader.
fn strip_alias(s: &str) -> String {
    let t = s.trim();
    match (t.find('('), t.ends_with(')')) {
        (Some(i), true) if t[i + 1..t.len() - 1].chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') => {
            t[..i].trim().to_string()
        }
        _ => t.to_string(),
    }
}

/// A cell, in the customer's words: `-` is "any", `not:` is "other than", and in Japanese a
/// comparison reads as 以下 / 以上 / 未満 / より大きい.
fn customer_cell(s: &str) -> String {
    let t = s.trim();
    if t == "-" {
        return tr!("どれでも", "any");
    }
    if let Some(r) = t.strip_prefix(crate::kw::NOT).and_then(|r| r.strip_prefix(':')) {
        return tr!("{}以外", "other than {}", r.trim());
    }
    if crate::i18n::ja() {
        // One bound reads as a word; two read as the lower one then the upper one
        // (`>10万円 <=50万円` → 「10万円より大きく 50万円以下」), the way the statute itself
        // says it. The two-bound cell used to come out as 「10万円 <=50万円より大きい」 (§15.71).
        let words: Vec<&str> = t.split_whitespace().collect();
        let bounds: Vec<(&str, &str)> = words
            .iter()
            .filter_map(|w| [">=", "<=", ">", "<"].iter().find_map(|op| w.strip_prefix(op).map(|v| (*op, v))))
            .collect();
        if !bounds.is_empty() && bounds.len() == words.len() {
            let word = |op: &str, last: bool| match (op, last) {
                ("<=", _) => "以下",
                (">=", _) => "以上",
                ("<", _) => "未満",
                (">", true) => "より大きい",
                (">", false) => "より大きく",
                _ => "",
            };
            let lower = bounds.iter().find(|(op, _)| *op == ">=" || *op == ">");
            let upper = bounds.iter().find(|(op, _)| *op == "<=" || *op == "<");
            return match (bounds.len(), lower, upper) {
                (1, _, _) => format!("{}{}", bounds[0].1, word(bounds[0].0, true)),
                (2, Some((lo, lv)), Some((ho, hv))) => format!("{lv}{} {hv}{}", word(lo, false), word(ho, true)),
                _ => t.to_string(),
            };
        }
    }
    t.to_string()
}

fn tax_words(tax: &str) -> String {
    if tax == crate::kw::INCL_TAX {
        tr!("税込", "tax included")
    } else if tax == crate::kw::EXCL_TAX {
        tr!("税抜", "tax excluded")
    } else {
        tax.to_string()
    }
}

fn rounding_words(r: &Rounding) -> String {
    let g = &r.grid.raw;
    match r.mode.as_str() {
        m if m == crate::kw::UP => tr!("{g}単位で切り上げ", "rounded up to {g}"),
        m if m == crate::kw::DOWN => tr!("{g}単位で切り捨て", "rounded down to {g}"),
        m if m == crate::kw::HALF_UP => tr!("{g}単位で四捨五入", "rounded half up to {g}"),
        m if m == crate::kw::HALF_EVEN => tr!("{g}単位で偶数への丸め", "rounded half to even at {g}"),
        m => format!("{m}({g})"),
    }
}

/// The same rounding as a sentence's verb: 「10円単位で切り上げます」.
fn rounding_verb(r: &Rounding) -> String {
    let g = &r.grid.raw;
    match r.mode.as_str() {
        m if m == crate::kw::UP => tr!("{g}単位で切り上げます", "rounded up to {g}"),
        m if m == crate::kw::DOWN => tr!("{g}単位で切り捨てます", "rounded down to {g}"),
        m if m == crate::kw::HALF_UP => tr!("{g}単位で四捨五入します", "rounded half up to {g}"),
        m if m == crate::kw::HALF_EVEN => tr!("{g}単位で偶数側に丸めます", "rounded half to even at {g}"),
        m => format!("{m}({g})"),
    }
}

/// What an input can be, for the customer: the values of a small enum, the name of a large
/// one, the range of a quantity with its unit, yes/no for a flag.
fn customer_input(c: &Checked, name: &str) -> String {
    let range = match (c.ty_of(name), c.ranges.get(name)) {
        (Some(Ty::Qty { unit, .. }), Some((lo, hi))) => {
            let s = |b: &Option<crate::num::Rat>| b.map(|v| qty_words(v, &unit)).unwrap_or_else(|| "…".into());
            match (lo, hi) {
                (None, None) => String::new(),
                _ => format!("{} 〜 {}", s(lo), s(hi)),
            }
        }
        _ => range_text(c, name),
    };
    match c.ty_of(name) {
        Some(Ty::Enum(e)) => {
            let vs = c.enums.get(&e).cloned().unwrap_or_default();
            if vs.len() <= 8 {
                tr!("{}のいずれか", "one of {}", vs.join(if crate::i18n::ja() { "・" } else { ", " }))
            } else {
                tr!("{e}のいずれか", "one of {e}")
            }
        }
        Some(Ty::Opt(inner)) => match *inner {
            Ty::Enum(e) => tr!("{e}のいずれか、または無し", "one of {e}, or none"),
            _ => tr!("任意", "optional"),
        },
        Some(Ty::Bool) => tr!("はい／いいえ", "yes / no"),
        Some(Ty::Money { tax, .. }) => {
            let t = tax.map(|x| tax_words(&x)).unwrap_or_default();
            match (range.is_empty(), t.is_empty()) {
                (true, true) => tr!("金額", "an amount"),
                (true, false) => tr!("金額（{t}）", "an amount ({t})"),
                (false, true) => range,
                (false, false) => tr!("{range}（{t}）", "{range} ({t})"),
            }
        }
        Some(Ty::Qty { .. }) | Some(Ty::Number) | Some(Ty::Rate) => {
            if range.is_empty() {
                tr!("数", "a number")
            } else {
                range
            }
        }
        Some(Ty::Date) => tr!("日付", "a date"),
        _ => String::new(),
    }
}

/// What an output is, for the customer.
fn customer_output(c: &Checked, name: &str) -> String {
    match c.ty_of(name) {
        Some(Ty::Enum(e)) => {
            let vs = c.enums.get(&e).cloned().unwrap_or_default();
            if vs.len() <= 8 {
                tr!("{}のいずれか", "one of {}", vs.join(if crate::i18n::ja() { "・" } else { ", " }))
            } else {
                tr!("{e}のいずれか", "one of {e}")
            }
        }
        Some(Ty::Bool) => tr!("はい／いいえ", "yes or no"),
        Some(Ty::Money { cur, tax }) => match tax.map(|x| tax_words(&x)) {
            Some(t) => tr!("{t}の金額（{cur}）", "an amount in {cur} ({t})"),
            None => tr!("金額（{cur}）", "an amount in {cur}"),
        },
        Some(Ty::Qty { unit, .. }) => tr!("量（{unit}）", "a quantity in {unit}"),
        Some(Ty::Rate) => tr!("率", "a rate"),
        Some(Ty::Number) => tr!("数", "a number"),
        Some(Ty::Date) => tr!("日付", "a date"),
        Some(Ty::Str) => tr!("文字列", "text"),
        _ => String::new(),
    }
}

/// A value with its unit, as the customer reads it.
fn customer_val(c: &Checked, name: &str, v: &crate::eval::Val) -> String {
    use crate::eval::Val;
    match (v, c.ty_of(name)) {
        (Val::Bool(b), _) => {
            if *b {
                tr!("はい", "yes")
            } else {
                tr!("いいえ", "no")
            }
        }
        (Val::Num(r), Some(Ty::Money { cur, .. })) => format!("{}{cur}", crate::types::fmt_big_pub(*r)),
        (Val::Num(r), Some(Ty::Qty { unit, .. })) => qty_words(*r, &unit),
        _ => crate::vectors::show_named(c, name, v),
    }
}

/// A quantity as a customer writes it: `20000g` is `20kg`, `1500g` stays `1500g`, and no
/// 万 multiplier is used on a unit (`2万g` is nobody's spelling).
fn qty_words(v: crate::num::Rat, unit: &str) -> String {
    if unit == "g" && v.den == 1 && v.num != 0 && v.num % 1000 == 0 {
        return format!("{}kg", v.num / 1000);
    }
    format!("{v}{unit}")
}

// ---------------------------------------------------------------------------
// The page the approver can try a case on (§15.37).
// ---------------------------------------------------------------------------

/// The same document as `render`, as one HTML file the approver can open and type a case
/// into. The markdown is converted here (it is our own, and a small subset), the rows of
/// every table get the table's name and their number on them, and the generated JavaScript
/// module — the same code `rulec gen` writes — runs in the page: type the inputs, and the
/// rows that matched light up, the outputs appear, and the record line the generated code
/// would log is shown. Nothing on the page is computed by anything but the generated code,
/// so it says nothing the code does not.
pub fn render_html(f: &RuleFile, c: &Checked, src: &str, path: &str, js: &str) -> String {
    let md = render(f, c, src, path);
    let body = md_to_html(&md);
    let title = tr!("規則 {} v{}", "Rule {} v{}", f.name.text, f.version);
    let lang = if crate::i18n::ja() { "ja" } else { "en" };
    let mut o = String::new();
    o.push_str(&format!(
        "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>\n{CSS}</style>\n</head>\n<body>\n<main>\n",
        html_esc(&title)
    ));
    o.push_str(&body.replace("<!--TRY-->", &try_panel()));
    o.push_str("</main>\n<script type=\"module\">\n");
    o.push_str(js);
    o.push_str(&format!("\nconst RULE = {};\n", rule_json(f, c)));
    let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
    o.push_str(&format!("const FN = {{ run: {alias}_traced, record: {alias}_record }};\n"));
    o.push_str(PAGE_JS);
    o.push_str("</script>\n</body>\n</html>\n");
    o
}

const CSS: &str = "\
body { font-family: system-ui, sans-serif; line-height: 1.5; max-width: 60rem; margin: 2rem auto; padding: 0 1rem; color: #222; background: #fff; }
table { border-collapse: collapse; margin: 0.5rem 0 1rem; }
th, td { border: 1px solid #c8c8c8; padding: 2px 10px; text-align: left; vertical-align: top; }
th { background: #f2f2f2; }
tr.hit td { background: #ffe9a8; }
code { background: #f4f4f4; padding: 0 3px; }
#try { border: 1px solid #c8c8c8; border-radius: 6px; padding: 12px 16px; margin: 1rem 0 1.5rem; background: #fafafa; }
#try h2 { margin-top: 0; }
#try-form { display: grid; grid-template-columns: max-content 1fr; gap: 6px 12px; align-items: center; max-width: 36rem; }
#try-form label { display: contents; }
#try-form input, #try-form select { font: inherit; padding: 2px 6px; }
#try-rows { margin: 10px 0; }
#try-rows table { margin: 4px 0; }
#try-rows input, #try-rows select, #try-rows button { font: inherit; padding: 2px 6px; }
#try-buttons { margin: 10px 0; display: flex; flex-wrap: wrap; gap: 8px; }
#try-buttons button { font: inherit; padding: 4px 12px; }
#try-result { font-weight: 600; margin: 8px 0; min-height: 1.5em; }
#try-record { font-family: ui-monospace, monospace; font-size: 0.85em; white-space: pre-wrap; word-break: break-all; color: #555; margin: 0; }
";

/// The panel's static part; the fields are built by the script from the rule's own
/// description, so the page never carries a second copy of the inputs.
fn try_panel() -> String {
    tr!(
        "<section id=\"try\">\n<h2>試してみる</h2>\n<p>入力を入れると、当てはまった行に色が付き、結果が出ます。動くのは生成コードそのもので、ログに書かれる一行もそのまま出ます。</p>\n<form id=\"try-form\"></form>\n<div id=\"try-rows\"></div>\n<div id=\"try-buttons\"></div>\n<div id=\"try-result\"></div>\n<pre id=\"try-record\"></pre>\n</section>\n",
        "<section id=\"try\">\n<h2>Try a case</h2>\n<p>Enter the inputs: the rows that match light up and the outputs appear. What runs is the generated code itself, and the line it would write to a log is shown as it is.</p>\n<form id=\"try-form\"></form>\n<div id=\"try-rows\"></div>\n<div id=\"try-buttons\"></div>\n<div id=\"try-result\"></div>\n<pre id=\"try-record\"></pre>\n</section>\n"
    )
}

/// What the script needs to know about the rule, as JSON: the inputs (kind, unit, range,
/// enum values, the wire scale of a rate), the outputs, and the examples in wire form.
fn rule_json(f: &RuleFile, c: &Checked) -> String {
    use crate::json::Obj;
    let kind_of = |ty: &Ty| -> &'static str {
        match ty {
            Ty::Enum(_) => "enum",
            Ty::Bool => "bool",
            Ty::Date => "date",
            Ty::Str => "string",
            Ty::Rate => "rate",
            Ty::Opt(_) => "string",
            _ => "number",
        }
    };
    let unit_of = |ty: &Ty| -> Option<String> {
        match ty {
            Ty::Money { cur, .. } => Some(cur.clone()),
            Ty::Qty { unit, .. } => Some(unit.clone()),
            Ty::Rate => Some("%".into()),
            _ => None,
        }
    };
    let one = |name: &str, alias: &str| -> String {
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        let mut o = Obj::new().str("name", name).str("alias", alias).str("kind", kind_of(&ty));
        if let Some(u) = unit_of(&ty) {
            o = o.str("unit", &u);
        }
        let sc = c.wire_scale(name);
        o = o.int("scale", sc);
        if let Some((Some(lo), Some(hi))) = c.ranges.get(name) {
            o = o.raw(
                "range",
                Obj::new()
                    .int("min", crate::types::wire_int(*lo, sc))
                    .int("max", crate::types::wire_int(*hi, sc))
                    .finish(),
            );
        }
        if let Ty::Enum(e) = &ty {
            if let Some(vs) = c.enums.get(e) {
                let vs: Vec<String> = vs.iter().map(|v| format!("{v:?}")).collect();
                o = o.raw("values", crate::json::arr(&vs));
            }
        }
        o.finish()
    };
    let ins: Vec<String> = f.inputs.iter().map(|i| one(&i.name.text, &crate::codegen::pub_name_of(&i.name))).collect();
    let outs: Vec<String> = f.outputs.iter().map(|o| one(&o.name.text, &crate::codegen::pub_name_of(&o.name))).collect();
    // The examples, as the wire values the script puts into the fields.
    let mut exs: Vec<String> = Vec::new();
    if let Some(t) = &f.examples {
        for row in &t.rows {
            let mut o = Obj::new();
            let mut complete = true;
            for (k, (col, _)) in t.inputs.iter().enumerate() {
                // The sequence column holds the name of a `sequence`, and what the panel
                // needs is its rows (§15.56).
                if f.elements.as_ref().is_some_and(|el| &el.name.text == col) {
                    match row.cells.get(k) {
                        Some(Cell::Lit(Lit::Word(w))) => match seq_json(f, c, w) {
                            Some(a) => o = o.raw(col, a),
                            None => complete = false,
                        },
                        _ => complete = false,
                    }
                    continue;
                }
                let ty = c.ty_of(col).unwrap_or(Ty::Unknown);
                let v = match row.cells.get(k) {
                    Some(Cell::Lit(l)) => crate::eval::lit_to_val(l, &ty),
                    _ => None,
                };
                match v.and_then(|v| crate::report::wire(c, col, Some(&v))) {
                    Some(w) => o = o.str(col, &w),
                    None => complete = false,
                }
            }
            if complete {
                exs.push(o.finish());
            }
        }
    }
    // The sequence a walk reads (§15.56): the panel builds a small editor for it, one row
    // per element, from the same field description an input gets.
    let elements = f.elements.as_ref().map(|el| {
        let fields: Vec<String> =
            el.fields.iter().map(|fd| one(&fd.name.text, &crate::codegen::pub_name_of(&fd.name))).collect();
        Obj::new()
            .str("name", &el.name.text)
            .str("alias", &crate::codegen::pub_name_of(&el.name))
            .raw("fields", crate::json::arr(&fields))
            .finish()
    });
    let mut o = Obj::new()
        // The rule names itself: the page says who it is when it introduces itself to a
        // host that is rendering it (SEP-1865).
        .str("name", &f.name.text)
        .str("version", &f.version)
        .raw("inputs", crate::json::arr(&ins))
        .raw("outputs", crate::json::arr(&outs))
        .raw("examples", crate::json::arr(&exs))
        .raw(
            "text",
            Obj::new()
                .str("run", &tr!("計算する", "Compute"))
                .str("example", &tr!("例", "Example"))
                .str("add", &tr!("行を足す", "Add a row"))
                .str("remove", &tr!("この行を消す", "Remove this row"))
                .finish(),
        );
    if let Some(e) = elements {
        o = o.raw("elements", e);
    }
    o.finish()
}

/// One named `sequence` as the panel reads it: an array of objects, each field in the same
/// wire form an example's scalar cell gets, so one `fromWire` fills them all (§15.56).
fn seq_json(f: &RuleFile, c: &Checked, name: &str) -> Option<String> {
    use crate::json::Obj;
    let sq = f.sequences.iter().find(|s| s.name.text == name)?;
    let mut rows: Vec<String> = Vec::new();
    for row in &sq.rows {
        let mut o = Obj::new();
        for (ci, (col, _)) in sq.cols.iter().enumerate() {
            let ty = c.ty_of(col).unwrap_or(Ty::Unknown);
            let Some(Cell::Lit(l)) = row.cells.get(ci) else { return None };
            let w = crate::eval::lit_to_val(l, &ty).and_then(|v| crate::report::wire(c, col, Some(&v)))?;
            o = o.str(col, &w);
        }
        rows.push(o.finish());
    }
    Some(crate::json::arr(&rows))
}

/// The script that drives the panel. It knows nothing about the rule beyond `RULE`.
const PAGE_JS: &str = r##"
const $ = (s) => document.querySelector(s);
const form = $("#try-form");
const dateOf = (days) => new Date(Number(days) * 86400000).toISOString().slice(0, 10);
function widget(inp) {
  let el;
  if (inp.kind === "enum") {
    el = document.createElement("select");
    for (const v of inp.values) {
      const o = document.createElement("option");
      o.value = v;
      o.textContent = v;
      el.appendChild(o);
    }
  } else if (inp.kind === "bool") {
    el = document.createElement("input");
    el.type = "checkbox";
  } else if (inp.kind === "date") {
    el = document.createElement("input");
    el.type = "date";
  } else if (inp.kind === "string") {
    el = document.createElement("input");
    el.type = "text";
  } else {
    el = document.createElement("input");
    el.type = "number";
    el.step = inp.kind === "rate" ? "any" : "1";
    if (inp.range) {
      const show = (v) => (inp.kind === "rate" ? String((v * 100) / inp.scale) : String(v));
      el.min = show(inp.range.min);
      el.max = show(inp.range.max);
      el.placeholder = show(inp.range.min) + " … " + show(inp.range.max);
    }
  }
  return el;
}
for (const inp of RULE.inputs) {
  const label = document.createElement("label");
  const span = document.createElement("span");
  span.textContent = inp.name + (inp.unit ? " (" + inp.unit + ")" : "");
  const el = widget(inp);
  el.name = inp.alias;
  label.appendChild(span);
  label.appendChild(el);
  form.appendChild(label);
}
// A rule that walks a sequence gets an editor for it: one row per element, with the same
// field widgets an input gets. The rows live outside the form so that a field name repeated
// down the column does not collide with `form.elements`.
let rows = [];
// Assigned when there is a sequence to edit; `fillRows` below is what reaches for it.
let addRow = null;
if (RULE.elements) {
  const box = $("#try-rows");
  const caption = document.createElement("div");
  caption.textContent = RULE.elements.name;
  box.appendChild(caption);
  const table = document.createElement("table");
  const head = document.createElement("tr");
  for (const fd of RULE.elements.fields) {
    const th = document.createElement("th");
    th.textContent = fd.name + (fd.unit ? " (" + fd.unit + ")" : "");
    head.appendChild(th);
  }
  head.appendChild(document.createElement("th"));
  const body = document.createElement("tbody");
  table.appendChild(head);
  table.appendChild(body);
  box.appendChild(table);
  addRow = (vals) => {
    const tr = document.createElement("tr");
    const cells = {};
    for (const fd of RULE.elements.fields) {
      const td = document.createElement("td");
      const el = widget(fd);
      if (vals && vals[fd.name] !== undefined) fromWire(fd, el, vals[fd.name]);
      td.appendChild(el);
      tr.appendChild(td);
      cells[fd.alias] = el;
    }
    const td = document.createElement("td");
    const rm = document.createElement("button");
    rm.type = "button";
    rm.textContent = "×";
    rm.title = RULE.text.remove;
    rm.addEventListener("click", () => {
      tr.remove();
      rows = rows.filter((r) => r.tr !== tr);
    });
    td.appendChild(rm);
    tr.appendChild(td);
    body.appendChild(tr);
    rows.push({ tr, cells });
  };
  const add = document.createElement("button");
  add.type = "button";
  add.textContent = RULE.text.add;
  add.addEventListener("click", () => addRow());
  box.appendChild(add);
  // One row to start with, so the shape is visible; removing it is the empty sequence,
  // which is a case of its own.
  addRow();
}
// An example of a walk carries its elements, so choosing it rebuilds the rows as well.
function fillRows(ex) {
  if (!RULE.elements) return;
  const es = ex[RULE.elements.name];
  if (!Array.isArray(es)) return;
  for (const r of rows.slice()) r.tr.remove();
  rows = [];
  for (const e of es) addRow(e);
}
function toWire(inp, el) {
  switch (inp.kind) {
    case "enum":
    case "string":
      return el.value;
    case "bool":
      return el.checked;
    case "date": {
      const [y, m, d] = el.value.split("-").map(Number);
      return BigInt(Math.round(Date.UTC(y, m - 1, d) / 86400000));
    }
    case "rate":
      return BigInt(Math.round((Number(el.value) * inp.scale) / 100));
    default:
      return BigInt(el.value === "" ? 0 : el.value);
  }
}
function fromWire(inp, el, v) {
  switch (inp.kind) {
    case "enum":
    case "string":
      el.value = v;
      break;
    case "bool":
      el.checked = v === "true";
      break;
    case "date":
      el.value = v;
      break;
    case "rate":
      el.value = String((Number(v) * 100) / inp.scale);
      break;
    default:
      el.value = v;
  }
}
function shown(o, v) {
  if (o.kind === "rate") return String((Number(v) * 100) / o.scale) + "%";
  if (o.kind === "bool") return v ? "true" : "false";
  if (o.kind === "date") return dateOf(v);
  return String(v) + (o.unit ? o.unit : "");
}
function run() {
  const args = RULE.inputs.map((inp) => toWire(inp, form.elements[inp.alias]));
  if (RULE.elements) {
    args.push(
      rows.map((r) => {
        const e = {};
        for (const fd of RULE.elements.fields) e[fd.alias] = toWire(fd, r.cells[fd.alias]);
        return e;
      })
    );
  }
  for (const tr of document.querySelectorAll("tr.hit")) tr.classList.remove("hit");
  try {
    const [out, trace] = FN.run(...args);
    const vals = RULE.outputs.length === 1 ? [out] : RULE.outputs.map((o) => out[o.alias]);
    $("#try-result").textContent = RULE.outputs.map((o, i) => o.name + " = " + shown(o, vals[i])).join("   ");
    for (const f of trace) {
      const tr = document.querySelector('tr[data-t="' + CSS.escape(f.table) + '"][data-r="' + f.row + '"]');
      if (tr) tr.classList.add("hit");
    }
    $("#try-record").textContent = FN.record(...args, out, trace, "");
    try {
      // The sequence is not put in the address: a link carries the scalar inputs.
      const q = new URLSearchParams();
      for (const inp of RULE.inputs) {
        const el = form.elements[inp.alias];
        q.set(inp.alias, inp.kind === "bool" ? String(el.checked) : inp.kind === "rate" ? String(toWire(inp, el)) : inp.kind === "date" ? el.value : el.value);
      }
      history.replaceState(null, "", "?" + q.toString() + location.hash);
    } catch (_) {
      // A page opened somewhere the address cannot be rewritten still works; it just cannot be linked.
    }
  } catch (e) {
    $("#try-result").textContent = e.message;
    $("#try-record").textContent = "";
  }
}
const buttons = $("#try-buttons");
const go = document.createElement("button");
go.type = "button";
go.textContent = RULE.text.run;
go.addEventListener("click", run);
buttons.appendChild(go);
function fill(ex) {
  for (const inp of RULE.inputs) {
    if (ex[inp.name] !== undefined) fromWire(inp, form.elements[inp.alias], ex[inp.name]);
  }
  fillRows(ex);
}
RULE.examples.forEach((ex, i) => {
  const b = document.createElement("button");
  b.type = "button";
  b.textContent = RULE.text.example + " " + (i + 1);
  b.addEventListener("click", () => {
    fill(ex);
    run();
  });
  buttons.appendChild(b);
});
form.addEventListener("submit", (e) => {
  e.preventDefault();
  run();
});
// The case is in the address: `?example=2` opens the page on the second example, and
// `?<alias>=<wire value>&…` on any case, so a link is enough to show someone one.
const params = new URLSearchParams(location.search);
const ex = RULE.examples[Number(params.get("example")) - 1];
if (ex) {
  fill(ex);
  run();
} else if (RULE.inputs.some((inp) => params.has(inp.alias))) {
  for (const inp of RULE.inputs) {
    if (params.has(inp.alias)) fromWire(inp, form.elements[inp.alias], params.get(inp.alias));
  }
  run();
}
// `#t-基本送料` lands on that table, after the run above has settled the page's height.
if (location.hash) {
  const target = document.getElementById(decodeURIComponent(location.hash.slice(1)));
  if (target) target.scrollIntoView();
}
// MCP Apps (SEP-1865). Inside a host's frame this page is the view of one call, so it says
// hello and then opens on the case the host hands it: the same fields, the same rows lit up,
// the same record line. Opened as a file it is the page it always was — nothing below runs.
if (window.parent !== window) {
  const post = (m) => window.parent.postMessage(m, "*");
  const open = (args) => {
    if (!args) return;
    fill(args);
    run();
  };
  window.addEventListener("message", (e) => {
    const m = e.data;
    if (!m || m.jsonrpc !== "2.0") return;
    if (m.id === 1 && (m.result || m.error)) {
      // The handshake is answered; the host may send the call from here on.
      post({ jsonrpc: "2.0", method: "ui/notifications/initialized" });
      return;
    }
    if (m.method === "ui/notifications/tool-input") open(m.params && m.params.arguments);
    if (m.method === "ui/notifications/tool-result") {
      const rec = m.params && m.params.structuredContent;
      open(rec && rec.in);
    }
  });
  post({
    jsonrpc: "2.0",
    id: 1,
    method: "ui/initialize",
    params: {
      protocolVersion: "2026-01-26",
      appCapabilities: { availableDisplayModes: ["inline", "fullscreen"] },
      clientInfo: { name: RULE.name, version: RULE.version },
    },
  });
}
"##;

fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Inline markdown of the subset the rendering uses: `code` and **bold**, over escaped text.
/// One pass, so that bold may hold a code span (`**`rulec check` が確かめたこと**` does).
fn inline_html(s: &str) -> String {
    let mut o = String::new();
    let mut code = false;
    let mut strong = false;
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let ch = cs[i];
        if ch == '`' {
            o.push_str(if code { "</code>" } else { "<code>" });
            code = !code;
            i += 1;
            continue;
        }
        if !code && ch == '*' && cs.get(i + 1) == Some(&'*') {
            o.push_str(if strong { "</strong>" } else { "<strong>" });
            strong = !strong;
            i += 2;
            continue;
        }
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c => o.push(c),
        }
        i += 1;
    }
    if code {
        o.push_str("</code>");
    }
    if strong {
        o.push_str("</strong>");
    }
    o
}

/// The markdown `render` writes, as HTML. Headings, paragraphs, bullet lists (one level of
/// nesting) and pipe tables are all it uses. A data table under a `表`/`Table` heading gets
/// the table's name and the row number on each row, which is what the script highlights.
fn md_to_html(md: &str) -> String {
    let mut o = String::new();
    let mut table_name: Option<String> = None;
    let mut first_h2 = true;
    let mut para: Vec<String> = Vec::new();
    // `> ` lines: the quoted text of a source fragment (§15.68).
    let mut quote: Vec<String> = Vec::new();
    fn flush_quote(o: &mut String, quote: &mut Vec<String>) {
        if !quote.is_empty() {
            o.push_str("<blockquote>\n");
            for l in quote.iter() {
                o.push_str(&format!("<p>{}</p>\n", inline_html(l)));
            }
            o.push_str("</blockquote>\n");
            quote.clear();
        }
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut list: Vec<(usize, String)> = Vec::new();

    fn flush_para(o: &mut String, para: &mut Vec<String>) {
        if !para.is_empty() {
            o.push_str(&format!("<p>{}</p>\n", inline_html(&para.join("\n"))));
            para.clear();
        }
    }
    fn flush_list(o: &mut String, list: &mut Vec<(usize, String)>) {
        if list.is_empty() {
            return;
        }
        o.push_str("<ul>\n");
        let mut nested = false;
        for (depth, text) in list.iter() {
            if *depth > 0 && !nested {
                o.push_str("<ul>\n");
                nested = true;
            } else if *depth == 0 && nested {
                o.push_str("</ul>\n");
                nested = false;
            }
            o.push_str(&format!("<li>{}</li>\n", inline_html(text)));
        }
        if nested {
            o.push_str("</ul>\n");
        }
        o.push_str("</ul>\n");
        list.clear();
    }
    fn flush_table(o: &mut String, rows: &mut Vec<Vec<String>>, table_name: &Option<String>) {
        if rows.is_empty() {
            return;
        }
        let numbered = rows[0].first().is_some_and(|h| h == "#");
        o.push_str("<table>\n<thead><tr>");
        for h in &rows[0] {
            o.push_str(&format!("<th>{}</th>", inline_html(h)));
        }
        o.push_str("</tr></thead>\n<tbody>\n");
        for (k, r) in rows.iter().skip(1).enumerate() {
            // The row's position, which is what the trace carries; the `#` cell may show a
            // label instead of the number.
            let attrs = match (numbered, table_name, r.first()) {
                (true, Some(t), Some(_)) => format!(" data-t=\"{}\" data-r=\"{}\"", html_esc(t), k + 1),
                _ => String::new(),
            };
            o.push_str(&format!("<tr{attrs}>"));
            for cell in r {
                o.push_str(&format!("<td>{}</td>", inline_html(cell)));
            }
            o.push_str("</tr>\n");
        }
        o.push_str("</tbody>\n</table>\n");
        rows.clear();
    }

    for line in md.lines() {
        let t = line.trim_end();
        if let Some(q) = t.strip_prefix("> ") {
            flush_para(&mut o, &mut para);
            flush_list(&mut o, &mut list);
            flush_table(&mut o, &mut rows, &table_name);
            quote.push(q.to_string());
            continue;
        }
        flush_quote(&mut o, &mut quote);
        if t.starts_with("<!--") {
            o.push_str(t);
            o.push('\n');
            continue;
        }
        if let Some(h) = t.strip_prefix("# ") {
            flush_para(&mut o, &mut para);
            flush_list(&mut o, &mut list);
            flush_table(&mut o, &mut rows, &table_name);
            o.push_str(&format!("<h1>{}</h1>\n", inline_html(h)));
            continue;
        }
        if let Some(h) = t.strip_prefix("## ") {
            flush_para(&mut o, &mut para);
            flush_list(&mut o, &mut list);
            flush_table(&mut o, &mut rows, &table_name);
            if first_h2 {
                o.push_str("<!--TRY-->\n");
                first_h2 = false;
            }
            // `## 表 X（policy …）` / `## Table X (policy …)` — the name is what the rows carry.
            table_name = h
                .strip_prefix("表 ")
                .map(|r| r.split('（').next().unwrap_or(r).to_string())
                .or_else(|| h.strip_prefix("Table ").map(|r| r.split(" (").next().unwrap_or(r).to_string()))
                // `## 節 X → 出力` / `## Clause X → output`: a clause fires as `{"table":X,"row":1}`.
                .or_else(|| h.strip_prefix("節 ").map(|r| r.split(" →").next().unwrap_or(r).to_string()))
                .or_else(|| h.strip_prefix("Clause ").map(|r| r.split(" →").next().unwrap_or(r).to_string()));
            // A table's heading can be linked to (`#t-基本送料`), which is also how a
            // screenshot lands on it.
            match &table_name {
                Some(t) => o.push_str(&format!("<h2 id=\"t-{}\">{}</h2>\n", html_esc(t), inline_html(h))),
                None => o.push_str(&format!("<h2>{}</h2>\n", inline_html(h))),
            }
            continue;
        }
        if t.starts_with('|') {
            flush_para(&mut o, &mut para);
            flush_list(&mut o, &mut list);
            if t.starts_with("|---") {
                continue;
            }
            let cells: Vec<String> = t
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().replace("\\|", "|"))
                .collect();
            rows.push(cells);
            continue;
        }
        flush_table(&mut o, &mut rows, &table_name);
        let stripped = t.trim_start();
        if let Some(item) = stripped.strip_prefix("- ") {
            flush_para(&mut o, &mut para);
            let depth = if t.starts_with(' ') { 1 } else { 0 };
            list.push((depth, item.to_string()));
            continue;
        }
        if t.is_empty() {
            flush_para(&mut o, &mut para);
            flush_list(&mut o, &mut list);
            continue;
        }
        flush_list(&mut o, &mut list);
        para.push(t.to_string());
    }
    flush_para(&mut o, &mut para);
    flush_list(&mut o, &mut list);
    flush_table(&mut o, &mut rows, &table_name);
    flush_quote(&mut o, &mut quote);
    o
}

