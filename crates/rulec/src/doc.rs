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
            "\n## 歩く列: {}\n\n一件ぶんの欄です。呼び出し側は、この欄のそろった要素を何件でも渡します。\n\n| 名前 | 型 | 範囲 | 注記 |\n|---|---|---|---|\n",
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

    // --- Derived values and definitions. The invisible axes.
    let derived: Vec<&DerivedDecl> =
        f.items.iter().filter_map(|i| if let Item::Derived(d) = i { Some(d) } else { None }).collect();
    let defines: Vec<&DefineDecl> =
        f.items.iter().filter_map(|i| if let Item::Define(d) = i { Some(d) } else { None }).collect();
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
        o.push_str(&tr!("\n## 例が歩く列\n\n", "\n## The sequences the examples walk\n\n"));
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
    // its own column, so drop it from the expression.
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
    with_notes(&mut head, &mut rows, lines, &row_lines);
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
        for r in rows.iter().skip(1) {
            let attrs = match (numbered, table_name, r.first()) {
                (true, Some(t), Some(n)) if n.chars().all(|c| c.is_ascii_digit()) => {
                    format!(" data-t=\"{}\" data-r=\"{n}\"", html_esc(t))
                }
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
                .or_else(|| h.strip_prefix("Table ").map(|r| r.split(" (").next().unwrap_or(r).to_string()));
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
    o
}

