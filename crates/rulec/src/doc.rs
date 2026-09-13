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

/// Take only the trailing comment from one line of the source file.
/// This is the only place where the origin of a provisional rounding (the §7.2 convention)
/// reaches the approver's eyes. It is also the breakwater against §16 item three (a placeholder
/// becoming the authority).
fn trailing_comment(lines: &[&str], line: usize) -> Option<String> {
    let l = lines.get(line.checked_sub(1)?)?;
    // A `#` inside a string literal is not picked up.
    let mut in_str = false;
    for (i, ch) in l.char_indices() {
        match ch {
            '"' => in_str = !in_str,
            '#' if !in_str => return Some(l[i + 1..].trim().to_string()),
            _ => {}
        }
    }
    None
}

/// Use the source line as it is. Rebuilding the cells from the AST could let the rendering
/// disagree with the source. To protect "traceability" by construction, the text always comes
/// from the source file.
fn source_cells(lines: &[&str], line: usize) -> Vec<String> {
    let Some(l) = lines.get(line.saturating_sub(1)) else { return Vec::new() };
    let body = match l.find('#') {
        Some(i) if l[..i].matches('|').count() >= 2 => &l[..i],
        _ => l,
    };
    let t = body.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

/// The line number of a table's header line (a `|` line containing `→`).
fn header_line(lines: &[&str], from: usize) -> Option<usize> {
    (from..=lines.len()).find(|&n| {
        let l = lines.get(n - 1).map(|s| s.trim()).unwrap_or("");
        l.starts_with('|') && l.contains('→')
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
        format!("この {n} 群")
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
    let s = |b: &Option<crate::num::Rat>| match b {
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
    for it in &f.items {
        match it {
            Item::Derived(d) if d.name.text == col => return tr!("導出", "Derived value"),
            Item::Define(d) if d.name.text == col => return tr!("定義", "Definition"),
            Item::Table(t) if t.outputs.iter().any(|o| o.name.text == col) => {
                let n = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                return tr!("表 {n} の出力", "Output of table {n}");
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
            "{g}は {en} の {all}を過不足なく分割しています（この描画が宣言から数えました）。",
            "{g} the {all} of {en} exactly (counted from the declarations by this rendering).",
            g = n_groups(n, "partition"),
            all = n_values(all.len())
        ),
        (true, false) => tr!(
            "{g}は {en} の {all}をすべて覆いますが、{dup} が二つ以上の群に属します（この描画が宣言から数えました）。",
            "{g} all {all} of {en}, but the following belong to two or more groups: {dup} (counted from the declarations by this rendering).",
            g = n_groups(n, "cover"),
            all = n_values(all.len()),
            dup = dup.join(sep())
        ),
        (false, true) => tr!(
            "{g}が覆うのは {en} の {all}のうち {seen}で、{miss} はどの群にも属しません（この描画が宣言から数えました）。",
            "{g} {seen} of the {all} of {en}; the following belong to no group: {miss} (counted from the declarations by this rendering).",
            g = n_groups(n, "cover"),
            all = n_values(all.len()),
            seen = n_values(seen.len()),
            miss = join_some(&missing, 8)
        ),
        (false, false) => tr!(
            "{g}は {en} の {all}のうち {seen}を覆い、{dup} が重複、{miss} が未収容です（この描画が宣言から数えました）。",
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
        "<!-- rulec {} が {path} (sha256:{}) から生成。これは読み取り専用の描画で、\
         正本は .rule のほうです。編集しても戻せません（§1.6）。 -->\n",
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
    if !f.enums.is_empty() || !f.imports.is_empty() {
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
        o.push_str(&tr!("\n## 群\n\n", "\n## Groups\n\n"));
        o.push_str(&tr!(
            "群は列挙の名前つき部分集合です。表のセルに書かれた一語が、下の値をまとめて指しています。\n\n",
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

    // --- Derived values and definitions. The invisible axes.
    let derived: Vec<&DerivedDecl> =
        f.items.iter().filter_map(|i| if let Item::Derived(d) = i { Some(d) } else { None }).collect();
    let defines: Vec<&DefineDecl> =
        f.items.iter().filter_map(|i| if let Item::Define(d) = i { Some(d) } else { None }).collect();
    if !derived.is_empty() || !defines.is_empty() {
        o.push_str(&tr!("\n## 導出と定義\n\n", "\n## Derived Values and Definitions\n\n"));
        o.push_str(&tr!(
            "表の列に置ける中間の値です。式は原本のとおりで、範囲は宣言されたものです。\n\n",
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
                md_esc(&trailing_comment(&lines, d.name.span.line).unwrap_or_default())
            ));
        }
        for d in &defines {
            let e = expr_src(&lines, d.name.span.line);
            o.push_str(&tr!(
                "| {} | 定義 | `{}` |  | {} |\n",
                "| {} | Definition | `{}` |  | {} |\n",
                md_esc(&d.name.text),
                md_esc(&e),
                md_esc(&trailing_comment(&lines, d.name.span.line).unwrap_or_default())
            ));
        }
        if !derived.is_empty() {
            o.push_str(&tr!(
                "\n導出の宣言範囲が、入力範囲から実際に到達しうる値をすべて含んでいることは `rulec check` が確かめました（E112）。\n",
                "\nThat the declared range of every derived value contains all the values actually reachable from the input ranges was verified by `rulec check` (E112).\n"
            ));
        }
    }

    // --- Tables
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        o.push_str(&table_section(f, c, t, &lines, path));
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
            o.push_str(&md_table(&source_cells(&lines, h), &ex.rows.iter().map(|r| source_cells(&lines, r.span.line)).collect::<Vec<_>>(), false));
        }
        o.push_str(&tr!(
            "\nこの {} 件は `rulec check` が参照評価器で実行し、すべて宣言どおりの値になりました（E107）。例は**実行される仕様**です。\n",
            "\n`rulec check` ran these {} examples through the reference evaluator, and every one produced the declared values (E107). The examples are an **executable specification**.\n",
            ex.rows.len()
        ));
    }

    o
}

/// The right-hand side of `=` on a declaration line, verbatim from the source file.
fn expr_src(lines: &[&str], line: usize) -> String {
    let Some(l) = lines.get(line.saturating_sub(1)) else { return String::new() };
    let body = match l.find('#') {
        Some(i) => &l[..i],
        None => l,
    };
    let rhs = match body.split_once('=') {
        Some((_, rhs)) => rhs,
        None => body,
    };
    // A derived-value declaration continues as `= expr  range >=… <=…`. The range is shown in
    // its own column, so drop it from the expression.
    match rhs.find(&format!(" {} ", crate::kw::RANGE)) {
        Some(i) => rhs[..i].trim().to_string(),
        None => rhs.trim().to_string(),
    }
}

fn md_table(head: &[String], rows: &[Vec<String>], numbered: bool) -> String {
    let mut o = String::from("|");
    if numbered {
        o.push_str(" # |");
    }
    for h in head {
        o.push_str(&format!(" {} |", md_esc(h)));
    }
    o.push_str("\n|");
    for _ in 0..head.len() + usize::from(numbered) {
        o.push_str("---|");
    }
    o.push('\n');
    for (i, r) in rows.iter().enumerate() {
        o.push('|');
        if numbered {
            o.push_str(&format!(" {} |", i + 1));
        }
        for k in 0..head.len() {
            o.push_str(&format!(" {} |", md_esc(r.get(k).map(|s| s.as_str()).unwrap_or(""))));
        }
        o.push('\n');
    }
    o
}

fn table_section(f: &RuleFile, c: &Checked, t: &Table, lines: &[&str], path: &str) -> String {
    let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
    let mut o = tr!("\n## 表 {name}（{} {policy}）\n\n", "\n## Table {name} ({} {policy})\n\n", crate::kw::POLICY);

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
    let rows: Vec<Vec<String>> = t.rows.iter().map(|r| source_cells(lines, r.span.line)).collect();
    o.push('\n');
    o.push_str(&md_table(&head, &rows, true));

    // The default row. Under `上から` (`first`), a last all-`-` row means "everything else gets
    // this".
    let default_row = t
        .rows
        .iter()
        .position(|r| !r.cells.is_empty() && r.cells.iter().all(|x| matches!(x, Cell::DontCare)));
    if let Some(i) = default_row {
        o.push_str(&tr!(
            "\n行{} はすべての列が `-` なので、上のどれにも当たらない入力が落ちる**既定行**です。\n",
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
            "\nこの表のセルに現れる群: {}。中身は「群」の節にあります。\n",
            "\nGroups appearing in the cells of this table: {}. Their members are in the \"Groups\" section.\n",
            list.join(sep())
        ));
    }

    // Verified statements. This part alone writes nothing but what `rulec check` verified.
    o.push_str(&tr!("\n**`rulec check` が確かめたこと**\n\n", "\n**What `rulec check` verified**\n\n"));
    o.push_str(&tr!(
        "- どの入力の組合せも、いずれかの行に当たります（E101 完全性）\n",
        "- Every combination of inputs matches some row (E101 completeness)\n"
    ));
    o.push_str(&tr!(
        "- 決して当たらない行はありません（E102 冗長）\n",
        "- There is no row that can never match (E102 unreachable row)\n"
    ));
    let r = region::check_table(t, c, f, path, region::DEFAULT_BUDGET);
    match t.policy {
        Policy::Unique => {
            if r.w114.is_empty() {
                o.push_str(&tr!(
                    "- 二つ以上の行に同時に当たる入力はありません（E105 重なり）。行の並べ替えは意味を変えません\n",
                    "- No input matches two or more rows at once (E105 overlap). Reordering the rows does not change the meaning\n"
                ));
            } else {
                let pairs: Vec<String> =
                    r.w114.iter().map(|(i, j)| tr!("行{} と 行{}", "row {} and row {}", i + 1, j + 1)).collect();
                o.push_str(&tr!(
                    "- 行の重なりは見つかりませんでしたが、{} の排他は**証明できていません**（W114）。\
                     万一その入力が来たとき黙って先の行を選ばないよう、生成コードに実行時の番人が入ります\n",
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
                    "- 行の重なりは {} 対あります（階段の通常の姿である**構造的** {}、\
                     どちらが勝っても値の変わらない**同値** {}、出力が食い違うので**要確認** {}）。\
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
                        .find(|n| n.starts_with(&tr!("両方に当たる例:", "Both rows match:")))
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
