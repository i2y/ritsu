//! 検査済みの描画（§1.6）。
//!
//! `.rule` は既にほぼ markdown なので、構文の転写には価値がない（`|---|` を一行
//! 挿むだけになる）。この面の仕事は、**検査器が知っていて字面に現れない事実を
//! 人間の言葉で添える**ことである。群の一語が 47 県のうち 6 県を隠していることを、
//! 承認する立場の人は目視では確かめられない。検査器は知っている。
//!
//! 読み手は承認者と、PR をレビューする業務担当者。規則を書く人は `.rule` を
//! 直接読むので対象ではない。§11 の五文面が書き手のための面だとすれば、
//! これは承認者のための最初の面になる。
//!
//! **禁則**：検査器の出力に無い文章は書かない。すべての行が、原本か検査結果に
//! 遡れること。要約や解説の創作を始めた瞬間、この面も「証明していないことを
//! 言う面」になる。事実の出どころは二つに分けて名乗る — `rulec check` が
//! 確かめたものと、この描画が宣言から数えたもの。
//!
//! 逆方向（markdown から `.rule`）は作らない。§1.4 と同じ強さの一方向原則。

use crate::ast::*;
use crate::region;
use crate::types::{Checked, Ty};
use std::collections::{BTreeMap, BTreeSet};

/// 原本の一行から、行末コメントだけを取り出す。
/// 丸めの仮置きの出典（§7.2 の慣行）が承認者の目に入る場所はここしかない。
/// §16 第三（仮置きの権威化）への防波堤でもある。
fn trailing_comment(lines: &[&str], line: usize) -> Option<String> {
    let l = lines.get(line.checked_sub(1)?)?;
    // 文字列リテラルの中の `#` は拾わない。
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

/// 原本の行をそのまま使う。セルを AST から組み直すと、原本と描画が食い違いうる。
/// 「遡れること」を仕組みで守るために、字面はいつも原本から取る。
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

/// 表の見出し行（`→` を含む `|` 行）の行番号。
fn header_line(lines: &[&str], from: usize) -> Option<usize> {
    (from..=lines.len()).find(|&n| {
        let l = lines.get(n - 1).map(|s| s.trim()).unwrap_or("");
        l.starts_with('|') && l.contains('→')
    })
}

fn md_esc(s: &str) -> String {
    s.replace('|', "\\|")
}

fn ty_text(c: &Checked, name: &str) -> String {
    match c.ty_of(name) {
        Some(Ty::Enum(e)) => {
            let n = c.enums.get(&e).map(|v| v.len()).unwrap_or(0);
            format!("{e}（{n} 値）")
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
    // 原本が `100万円` と書いたものを `1000000円` と見せない（§1.6、§2.1 と同じ書き戻し）。
    let s = |b: &Option<crate::num::Rat>| match b {
        Some(v) => format!("{}{unit}", crate::types::fmt_big_pub(*v)),
        None => "…".into(),
    };
    match (lo, hi) {
        (None, None) => String::new(),
        _ => format!("{} 〜 {}", s(lo), s(hi)),
    }
}

/// 列がどこから来るか。承認者が「この値は誰が決めたのか」を辿れるようにする。
fn producer(f: &RuleFile, col: &str) -> String {
    if f.inputs.iter().any(|i| i.name.text == col) {
        return "入力".into();
    }
    for it in &f.items {
        match it {
            Item::Derived(d) if d.name.text == col => return "導出".into(),
            Item::Define(d) if d.name.text == col => return "定義".into(),
            Item::Table(t) if t.outputs.iter().any(|o| o.name.text == col) => {
                let n = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                return format!("表 {n} の出力");
            }
            _ => {}
        }
    }
    String::new()
}

/// 群がその列挙をどう覆っているか。**言えることだけ言う**。
/// 分割になっていなければ、なっていないと言う。
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
        (true, true) => format!(
            "この {n} 群は {en} の {} 値を過不足なく分割しています（この描画が宣言から数えました）。",
            all.len()
        ),
        (true, false) => format!(
            "この {n} 群は {en} の {} 値をすべて覆いますが、{} が二つ以上の群に属します（この描画が宣言から数えました）。",
            all.len(),
            dup.join("、")
        ),
        (false, true) => format!(
            "この {n} 群が覆うのは {en} の {} 値のうち {} 値で、{} はどの群にも属しません（この描画が宣言から数えました）。",
            all.len(),
            seen.len(),
            join_some(&missing, 8)
        ),
        (false, false) => format!(
            "この {n} 群は {en} の {} 値のうち {} 値を覆い、{} が重複、{} が未収容です（この描画が宣言から数えました）。",
            all.len(),
            seen.len(),
            dup.join("、"),
            join_some(&missing, 8)
        ),
    }
}

fn join_some(v: &[&str], max: usize) -> String {
    if v.len() <= max {
        return v.join("、");
    }
    format!("{}（ほか {} 件）", v[..max].join("、"), v.len() - max)
}

/// check を通った規則を描画する。通らない規則は呼び出し側が弾く
/// （壊れた規則の綺麗な描画は嘘になる。§1.6）。
pub fn render(f: &RuleFile, c: &Checked, src: &str, path: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let hash = crate::codegen::hash(src);
    let mut o = String::new();

    // 古い描画が正の顔をして残るのが最大の危険なので、原本のパス・ハッシュ・
    // 道具の版を刻む。貼られた先でも古さを検査できる。
    o.push_str(&format!(
        "<!-- rulec {} が {path} (sha256:{}) から生成。これは読み取り専用の描画で、\
         正本は .rule のほうです。編集しても戻せません（§1.6）。 -->\n",
        env!("CARGO_PKG_VERSION"),
        &hash[..12]
    ));
    o.push_str(&format!("# 規則 {} v{}\n", f.name.text, f.version));
    if let Some(d) = &f.description {
        o.push_str(&format!("\n{d}\n"));
    }

    // --- 入力
    o.push_str("\n## 入力\n\n| 名前 | 型 | 範囲 | 注記 |\n|---|---|---|---|\n");
    for i in &f.inputs {
        let n = &i.name.text;
        let mut note: Vec<String> = Vec::new();
        if i.contract_only {
            note.push("`契約のみ`（範囲の入口検査にだけ使い、表の条件には現れません）".into());
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

    // --- 出力
    o.push_str("\n## 出力\n\n| 名前 | 型 | 丸め | 注記 |\n|---|---|---|---|\n");
    for od in &f.outputs {
        let n = &od.name.text;
        let r = od.rounding.as_ref().map(|r| format!("{}({})", r.mode, r.grid.raw)).unwrap_or_default();
        // 丸めの仮置きの出典は、ここでしか承認者の目に入らない（§7.2、§16 第三）。
        let note = trailing_comment(&lines, od.name.span.line).unwrap_or_default();
        o.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            md_esc(n),
            md_esc(&ty_text(c, n)),
            md_esc(&r),
            md_esc(&note)
        ));
    }

    // --- 型。閉じた列挙。値が増えたら網羅性検査が既存の表を割る。
    if !f.enums.is_empty() || !f.imports.is_empty() {
        o.push_str("\n## 型\n\n");
        o.push_str("列挙は**閉じた**有限集合です。値を足すと、それを見ていない表が完全性検査で割れます。\n\n");
        for e in &f.enums {
            let vs: Vec<String> = e
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    // §2.1 の `既定扱い`：専用の行を持たず既定の行に落ちるのが意図、
                    // という宣言。承認者にはこの意図こそ読ませたい。
                    if *e.default_marks.get(i).unwrap_or(&false) {
                        format!("{}（既定扱い）", v.text)
                    } else {
                        v.text.clone()
                    }
                })
                .collect();
            o.push_str(&format!(
                "- **{}**（{} 値）— {}\n",
                md_esc(&e.name.text),
                e.values.len(),
                md_esc(&vs.join("、"))
            ));
        }
        for (im, _) in &f.imports {
            let name = im.rsplit('/').next().unwrap_or(im);
            let n = c.enums.get(name).map(|v| v.len()).unwrap_or(0);
            o.push_str(&format!("- **{}**（{} 値）— 組み込み（`取込 {}`）\n", md_esc(name), n, md_esc(im)));
        }
        if f.enums.iter().any(|e| e.default_marks.iter().any(|b| *b)) {
            o.push_str("\n`既定扱い` は「この値は専用の行を持たず、既定の行に落ちるのが意図です」という宣言です。付いていない値が\
                        どの行にも名指しされていなければ、`rulec check` が書き忘れとして問います（W111）。\n");
        }
    }

    // --- 群。一語が何を隠しているか。
    if !f.groups.is_empty() {
        o.push_str("\n## 群\n\n");
        o.push_str("群は列挙の名前つき部分集合です。表のセルに書かれた一語が、下の値をまとめて指しています。\n\n");
        let mut by_enum: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for g in &f.groups {
            let en = c.groups.get(&g.name.text).map(|(e, _)| e.clone()).unwrap_or_default();
            by_enum.entry(en).or_default().push(g.name.text.clone());
            let ms: Vec<String> = g.members.iter().map(|m| m.text.clone()).collect();
            o.push_str(&format!(
                "- **{}**（{} 値）— {}\n",
                md_esc(&g.name.text),
                ms.len(),
                md_esc(&ms.join("、"))
            ));
        }
        for (en, names) in &by_enum {
            o.push_str(&format!("\n{}\n", group_fact(c, en, names)));
        }
    }

    // --- 導出と定義。見えない軸。
    let derived: Vec<&DerivedDecl> =
        f.items.iter().filter_map(|i| if let Item::Derived(d) = i { Some(d) } else { None }).collect();
    let defines: Vec<&DefineDecl> =
        f.items.iter().filter_map(|i| if let Item::Define(d) = i { Some(d) } else { None }).collect();
    if !derived.is_empty() || !defines.is_empty() {
        o.push_str("\n## 導出と定義\n\n");
        o.push_str("表の列に置ける中間の値です。式は原本のとおりで、範囲は宣言されたものです。\n\n");
        o.push_str("| 名前 | 種類 | 式 | 範囲 | 注記 |\n|---|---|---|---|---|\n");
        for d in &derived {
            let e = expr_src(&lines, d.name.span.line);
            o.push_str(&format!(
                "| {} | 導出 | `{}` | {} | {} |\n",
                md_esc(&d.name.text),
                md_esc(&e),
                md_esc(&range_text(c, &d.name.text)),
                md_esc(&trailing_comment(&lines, d.name.span.line).unwrap_or_default())
            ));
        }
        for d in &defines {
            let e = expr_src(&lines, d.name.span.line);
            o.push_str(&format!(
                "| {} | 定義 | `{}` |  | {} |\n",
                md_esc(&d.name.text),
                md_esc(&e),
                md_esc(&trailing_comment(&lines, d.name.span.line).unwrap_or_default())
            ));
        }
        if !derived.is_empty() {
            o.push_str("\n導出の宣言範囲が、入力範囲から実際に到達しうる値をすべて含んでいることは `rulec check` が確かめました（E112）。\n");
        }
    }

    // --- 表
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        o.push_str(&table_section(f, c, t, &lines, path));
    }

    // --- 結果
    if f.result.is_some() {
        if let Some(r) = &f.result {
            o.push_str(&format!("\n## 結果\n\n`{}`\n", md_esc(&expr_src(&lines, r.span.line))));
        }
    }

    // --- 例
    if let Some(ex) = &f.examples {
        o.push_str("\n## 例（検証済み）\n\n");
        if let Some(h) = header_line(&lines, ex.rows.first().map(|r| r.span.line).unwrap_or(1).saturating_sub(1)) {
            o.push_str(&md_table(&source_cells(&lines, h), &ex.rows.iter().map(|r| source_cells(&lines, r.span.line)).collect::<Vec<_>>(), false));
        }
        o.push_str(&format!(
            "\nこの {} 件は `rulec check` が参照評価器で実行し、すべて宣言どおりの値になりました（E107）。例は**実行される仕様**です。\n",
            ex.rows.len()
        ));
    }

    o
}

/// 宣言行の `=` の右側を、原本の字面のまま取る。
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
    // 導出の宣言は `= 式  範囲 >=… <=…` と続く。範囲は別の欄に出すので式から外す。
    match rhs.find(" 範囲 ") {
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
    let policy = if t.policy == Policy::Unique { "一意" } else { "上から" };
    let mut o = format!("\n## 表 {name}（方式 {policy}）\n\n");

    // どの値がどこから来て、どこへ流れるか。
    o.push_str("| 列 | 出どころ |\n|---|---|\n");
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
                    Some(format!("表 {}", x.name.as_ref().map(|q| q.text.clone()).unwrap_or_default()))
                }
                _ => None,
            })
            .collect();
        let dest = if f.outputs.iter().any(|q| q.name.text == *n) {
            "この規則の出力".to_string()
        } else if uses.is_empty() {
            "（この表の中だけ）".to_string()
        } else {
            format!("→ {} の入力列", uses.join("、"))
        };
        o.push_str(&format!("| → {} | {} |\n", md_esc(n), md_esc(&dest)));
    }

    let head_l = header_line(lines, t.span.line).unwrap_or(t.span.line);
    let mut head = source_cells(lines, head_l);
    // 出力列の見出しは、原本の型注記のままだと長い（`→ サイズ(size) : サイズ区分`）。
    // 単位・税区分・丸め格子を畳んで、承認者が読む形にする。畳んだ中身はすべて
    // 宣言から取っているので、原本に遡れる。
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
            format!("→ {n}（{}）", bits.join(" / "))
        };
    }
    let rows: Vec<Vec<String>> = t.rows.iter().map(|r| source_cells(lines, r.span.line)).collect();
    o.push('\n');
    o.push_str(&md_table(&head, &rows, true));

    // 既定行。`上から` の最後の全 `-` 行が「それ以外は全部これ」を意味する。
    let default_row = t
        .rows
        .iter()
        .position(|r| !r.cells.is_empty() && r.cells.iter().all(|x| matches!(x, Cell::DontCare)));
    if let Some(i) = default_row {
        o.push_str(&format!(
            "\n行{} はすべての列が `-` なので、上のどれにも当たらない入力が落ちる**既定行**です。\n",
            i + 1
        ));
    }

    // 群がセルに現れていれば、どれを見ればよいかを言う。
    // 群がセルに現れたか、そして `以外:` の形で現れたか。補集合の件数を添えるのは
    // 後者だけにする（使っていない形の規模感を出しても雑音になる）。
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
        // §1.6 条件: `以外: 群` は展開しないが、**件数は添える**。41 県の羅列は
        // 目視の確認に耐えないので展開しないのが正しいが、「補集合です」だけでは
        // 承認者の一次の問い（規模感が妥当か）に答えられない。基数は宣言から数えられる。
        let n = |g: &str| -> Option<(usize, usize)> {
            let (en, ms) = c.groups.get(g)?;
            Some((ms.len(), c.enums.get(en)?.len()))
        };
        let list: Vec<String> = used
            .iter()
            .map(|(g, negated)| match (n(g), negated) {
                (Some((k, all)), true) => {
                    format!("**{g}**（{k} 値、`以外: {g}` は残り {} 値）", all - k)
                }
                (Some((k, _)), false) => format!("**{g}**（{k} 値）"),
                _ => format!("**{g}**"),
            })
            .collect();
        o.push_str(&format!(
            "\nこの表のセルに現れる群: {}。中身は「群」の節にあります。\n",
            list.join("、")
        ));
    }

    // 検査済みの言明。ここだけは `rulec check` が確かめたことしか書かない。
    o.push_str("\n**`rulec check` が確かめたこと**\n\n");
    o.push_str("- どの入力の組合せも、いずれかの行に当たります（E101 完全性）\n");
    o.push_str("- 決して当たらない行はありません（E102 冗長）\n");
    let r = region::check_table(t, c, f, path, region::DEFAULT_BUDGET);
    match t.policy {
        Policy::Unique => {
            if r.w114.is_empty() {
                o.push_str("- 二つ以上の行に同時に当たる入力はありません（E105 重なり）。行の並べ替えは意味を変えません\n");
            } else {
                let pairs: Vec<String> =
                    r.w114.iter().map(|(i, j)| format!("行{} と 行{}", i + 1, j + 1)).collect();
                o.push_str(&format!(
                    "- 行の重なりは見つかりませんでしたが、{} の排他は**証明できていません**（W114）。\
                     万一その入力が来たとき黙って先の行を選ばないよう、生成コードに実行時の番人が入ります\n",
                    pairs.join("、")
                ));
            }
        }
        Policy::TopDown => {
            let s = r.shadow;
            if s.total() > 0 {
                o.push_str(&format!(
                    "- 行の重なりは {} 対あります（階段の通常の姿である**構造的** {}、\
                     どちらが勝っても値の変わらない**同値** {}、出力が食い違うので**要確認** {}）。\
                     方式 上から なので、先に書かれた行が勝ちます\n",
                    s.total(),
                    s.structural,
                    s.equivalent,
                    s.confirm
                ));
                // 要確認だけは、どの行対かと証人を出す。承認者が「意図通りですか」に
                // 答えられるのはここだけで、件数だけでは答えようがない。
                for d in r.diags.iter().filter(|d| d.code == "W105") {
                    let w = d
                        .notes
                        .iter()
                        .find(|n| n.starts_with("両方に当たる例:"))
                        .cloned()
                        .unwrap_or_default();
                    o.push_str(&format!("  - 要確認: {}。{}\n", md_esc(&d.title.replacen("行の重なり: ", "", 1)), md_esc(&w)));
                }
            } else {
                o.push_str("- 行の重なりはありません。`方式 一意` にすれば、並べ替えが意味を変えないことを検査が保証できます（W110）\n");
            }
        }
    }
    o
}
