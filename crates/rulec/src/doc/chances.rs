//! The random inputs and the `chances` sections on the pages (§15.224): for the reader who checks
//! the rule, what each random input is drawn from and each claim as written beside what `check`
//! counted for it; for the customer, the claims with their percentages; on the HTML page, the
//! chances of the case entered, from the generated function that counts them (§15.225).
//!
//! As everywhere on these pages, every line traces back to the file or to the check: what an input
//! is drawn from is its declaration, and what a claim came to is the count the check made (E133 and
//! W129 read the same count).

use super::{header_line, md_esc, md_table, range_text, source_cells, with_notes};
use crate::ast::{ChanceTest, FoldDecl, RuleFile};
use crate::chances::{Counted, Verdict, chance_text, percent_text};
use crate::num::Rat;
use crate::types::{Checked, Ty};

/// What a random input is drawn from, in a few words: `drawn uniformly from the 14 whole numbers
/// from 0 to 13`. `None` for an input that is not random.
pub(super) fn drawn_from(f: &RuleFile, c: &Checked, name: &str) -> Option<String> {
    if !f.is_random(name) {
        return None;
    }
    let n = crate::types::fmt_big_pub(Rat::int(crate::chances::points_of(c, name).unwrap_or(0) as i128));
    let range = range_text(c, name);
    let ty = c.ty_of(name);
    let scale = *c.scales.get(name).unwrap_or(&1);
    let mut text = match ty {
        Some(Ty::Bool) => tr!("true と false から一様に選ばれます（どちらも 1/2）", "drawn uniformly from true and false (each 1/2)"),
        Some(Ty::Enum(_)) => tr!("値 {n} 個から一様に選ばれます", "drawn uniformly from its {n} values"),
        Some(Ty::Number) if scale == 1 => tr!("{range} の整数 {n} 通りから一様に選ばれます", "drawn uniformly from the {n} whole numbers from {range}"),
        t => {
            let unit = match t {
                Some(Ty::Money { cur, .. }) => cur,
                Some(Ty::Qty { unit, .. }) => unit,
                _ => String::new(),
            };
            let step = format!("{}{unit}", chance_step(scale));
            tr!("{range} を {step} 刻みにした {n} 通りから一様に選ばれます", "drawn uniformly from the {n} values from {range}, in steps of {step}")
        }
    };
    // A constraint that ties a random input narrows the draw to the points that meet it (§15.224).
    if f.constraints.iter().any(|k| k.names().any(|x| x == name)) {
        text.push_str(&tr!("（制約を満たす点の中で）", " (among the points the constraints let in)"));
    }
    Some(format!("`{}`: {text}", crate::kw::RANDOM))
}

/// One step of a value held at `scale`: `1`, `0.01`.
fn chance_step(scale: i128) -> String {
    crate::types::fmt_big_pub(Rat::new(1, scale.max(1)))
}

/// What a claim came to, for the column beside it: `holds: 5/14 (35.7%)`.
fn verdict_text(cl: &crate::chances::Claim) -> String {
    match &cl.verdict {
        Verdict::Holds => {
            if let Some(p) = cl.chance {
                return tr!("成り立つ: {}（{}）", "holds: {} ({})", chance_text(p), percent_text(p));
            }
            // The chance moves from one combination to another: the lowest and the highest the
            // count found, when every class has one.
            let mut seen: Vec<Rat> = Vec::new();
            let mut every = true;
            for k in &cl.classes {
                match &k.how {
                    Counted::Boxes { total, hits, .. } if *total > 0 => seen.push(Rat::new(*hits as i128, *total as i128)),
                    Counted::Boxes { .. } | Counted::Empty => {}
                    Counted::Points { low: Some(lo), high: Some(hi), .. } => {
                        seen.push(*lo);
                        seen.push(*hi);
                    }
                    Counted::Points { points, .. } => every &= *points == 0,
                }
            }
            seen.sort_by(|a, b| a.cmp_to(*b));
            match (every, seen.first(), seen.last()) {
                (true, Some(lo), Some(hi)) if lo.cmp_to(*hi) != std::cmp::Ordering::Equal => tr!(
                    "成り立つ: {}（{}）〜 {}（{}）",
                    "holds: from {} ({}) to {} ({})",
                    chance_text(*lo),
                    percent_text(*lo),
                    chance_text(*hi),
                    percent_text(*hi)
                ),
                _ => tr!("どの組み合わせでも成り立つ", "holds for every combination"),
            }
        }
        Verdict::Broken { .. } => tr!("成り立たない（E133）", "does not hold (E133)"),
        Verdict::Undecided(_) => tr!("予算の中で決まらない（W129）", "not decided within the budget (W129)"),
    }
}

/// The random inputs, in one sentence each, for the head of a `chances` section.
fn drawn_lines(f: &RuleFile, c: &Checked) -> String {
    let mut o = tr!(
        "乱数の入力は、呼び出し側が、ほかの入力と独立に引きます。\n\n",
        "The random inputs are drawn by the caller, independently of the other inputs.\n\n"
    );
    for i in f.random_inputs() {
        let n = &i.name.text;
        if let Some(t) = drawn_from(f, c, n) {
            let t = t.split_once(": ").map(|(_, t)| t.to_string()).unwrap_or(t);
            o.push_str(&format!("- **{}**: {}\n", md_esc(n), md_esc(&t)));
        }
    }
    o
}

/// The `chances` sections, for the page that checks the rule (§1.6).
pub(super) fn sections(f: &RuleFile, c: &Checked, lines: &[&str]) -> String {
    if f.chances.is_empty() {
        return String::new();
    }
    let a = crate::chances::analyze(f, c, crate::region::DEFAULT_BUDGET as usize);
    let mut o = tr!("\n## 確率の主張（検証済み）\n\n", "\n## Chances (verified)\n\n");
    o.push_str(&drawn_lines(f, c));
    o.push_str(&tr!(
        "\n行はそれぞれ、入力のセルに当てはまる乱数でない入力の**どの組み合わせでも**、乱数の入力を一様に引いたとき、出力が出力のセルに当てはまる確率がその値であること（比べる形なら、それを満たすこと）を主張します。書いていない出力は、どの値でもかまいません。\n\n",
        "\nEach row claims that for **every** combination of the inputs that are not random its cells let in, with the random inputs drawn uniformly, the chance that the outputs match its output cells is the value written (or passes the comparisons written). An output with no column may be anything.\n\n"
    ));
    let mut points: u128 = 0;
    for (ti, ch) in f.chances.iter().enumerate() {
        let Some(h) = header_line(lines, ch.span.line + 1) else { continue };
        if ti > 0 {
            o.push('\n');
        }
        let mut head: Vec<String> = source_cells(lines, h).into_iter().map(|x| x.replacen("->", "→", 1)).collect();
        let mut rows: Vec<Vec<String>> = ch.rows.iter().map(|r| source_cells(lines, r.span.line)).collect();
        for (row, r) in rows.iter_mut().zip(&ch.rows) {
            let cl = a.claims.iter().find(|x| x.table == ti && x.row == r.index);
            row.resize(head.len(), String::new());
            row.push(cl.map(verdict_text).unwrap_or_default());
            points += cl.map(|x| x.points).unwrap_or(0);
        }
        head.push(tr!("検査の結果", "Checked"));
        let row_lines: Vec<usize> = ch.rows.iter().map(|r| r.span.line).collect();
        with_notes(&mut head, &mut rows, lines, &row_lines);
        o.push_str(&md_table(&head, &rows, false));
    }
    // How the claims were counted, as the check counted them: box by box, point by point, or from
    // the weights of a pick.
    let boxed = a.claims.iter().flat_map(|c| &c.classes).any(|k| matches!(k.how, Counted::Boxes { .. }));
    let weighted = matches!(crate::chances::method(f), crate::chances::Method::Weighted { .. });
    o.push('\n');
    if boxed {
        o.push_str(&tr!(
            "`rulec check` は、乱数でない入力と乱数の入力を規則自身の境目で区分に切り、区分の点の数を区間の長さの掛け算で数えました。",
            "`rulec check` cut the inputs that are not random and the random ones at the rule's own boundaries, and counted the points of each box as the product of the lengths of its intervals. "
        ));
    }
    if points > 0 {
        o.push_str(&match (weighted, boxed) {
            (true, _) => tr!(
                "重みで選ぶ規則なので、確率は重みから数えました。重みを足した要素ごとに、規則を一度だけ評価しています（この表の主張で {points} 回）。",
                "The rule picks by weight, so the chances were counted from the weights: the rule was run once for each element weighed ({points} runs for these claims). "
            ),
            (false, true) => tr!(
                "答えが区分の中で変わるところと、二種類の入力を一緒に読むところは、点を一つずつ評価しました（この表の主張で {points} 点）。",
                "Where an answer moves inside a box, or the two kinds of input are read together, it ran the points one by one ({points} points for these claims). "
            ),
            (false, false) => tr!(
                "答えが乱数の入力とともに変わるか、二種類の入力を一緒に読むので、`rulec check` は、セルに当てはまる乱数でない入力の組み合わせごとに、乱数の入力の点を一つずつ評価しました（この表の主張で {points} 点）。",
                "An answer moves with the random inputs, or the two kinds of input are read together, so `rulec check` ran the points of the random inputs one by one for each combination of the other inputs the cells let in ({points} points for these claims). "
            ),
        });
    }
    o.push_str(&tr!(
        "成り立たない主張は検査を止め（E133）、予算の中で決められなかった主張はそう言います（W129）。\n",
        "A claim that does not hold stops the check (E133), and one it could not decide within its budget is said to be undecided (W129).\n"
    ));
    let name = crate::codegen::chances_fn_name(&crate::codegen::pub_name_of(&f.name), "python");
    o.push_str(&tr!(
        "\n生成コードでは、呼び出しごとの答えの数え上げを `{name}`（Python）が返します。答えごとに、それを出す引き方の数と、引き方の全部の数です。ほかの言語での名前は `rulec api` が挙げます。\n",
        "\nThe generated code counts the answers of a call with `{name}` (in Python): each answer with how many draws give it, and how many draws there are. `rulec api` gives its name in the other languages.\n"
    ));
    o
}

/// What a pick by weight does (§15.226), under its fold's table of moves.
pub(super) fn weighted(fold: &FoldDecl) -> String {
    let Some(d) = &fold.draw else { return String::new() };
    if !fold.weighted() {
        return String::new();
    }
    tr!(
        "\n`{tw}` は、その判定に落ちた要素の重みを順に足していき、**{r}** が足した合計を初めて下回った要素の値を採ります。要素が採られる確率は、その重みを重みの合計で割った値です。**{r}** は重みの合計より小さくなければならず、生成コードはそうでない呼び出しを受け付けません。重みの合計が 0 なら、答えは `{ex}` です。\n",
        "\n`{tw}` adds the weight of each element that lands on its verdict to a running total, in order, and takes the value of the first element at which **{r}** is below that total: an element is taken with the chance of its weight over the total of the weights. **{r}** has to be below the total of the weights, and the generated code refuses a call where it is not; when the weights add up to 0, the answer is `{ex}`.\n",
        tw = crate::kw::TAKE_WEIGHTED,
        r = md_esc(&d.text),
        ex = crate::kw::EXHAUSTED
    )
}

/// What `held` is, for a fold with a pick by weight: the value taken, or the `empty` answer.
pub(super) fn weighted_held(fold: &FoldDecl, customer: bool) -> Option<String> {
    if !fold.weighted() {
        return None;
    }
    Some(if customer {
        tr!("\n`{h}` は、採った値、無ければ一件も無いときの答えです。\n", "\n`{h}` is the value taken, else the answer for no elements.\n", h = crate::kw::HELD)
    } else {
        tr!(
            "`{h}` は、採った値（`{tw}`）、無ければ `{e}` の答えです。\n",
            "`{h}` is the value taken (`{tw}`), else the `{e}` answer.\n",
            h = crate::kw::HELD,
            tw = crate::kw::TAKE_WEIGHTED,
            e = crate::kw::EMPTY
        )
    })
}

/// The claims, for the customer: each row as written, with its chance as a percentage.
pub(super) fn customer(f: &RuleFile, c: &Checked, lines: &[&str]) -> String {
    if f.chances.is_empty() {
        return String::new();
    }
    let mut o = tr!("\n## 答えごとの確率\n\n", "\n## How likely each answer is\n\n");
    for i in f.random_inputs() {
        if let Some(t) = drawn_from(f, c, &i.name.text) {
            let t = t.split_once(": ").map(|(_, t)| t.to_string()).unwrap_or(t);
            o.push_str(&format!("- **{}**: {}\n", md_esc(&i.name.text), md_esc(&t)));
        }
    }
    o.push_str(&tr!(
        "\n表の行の場合には、答えがその確率で出ます（rulec が数えて確かめています）。\n\n",
        "\nIn the cases a row describes, the answer comes out with the chance written (rulec counted it to check).\n\n"
    ));
    for (ti, ch) in f.chances.iter().enumerate() {
        let Some(h) = header_line(lines, ch.span.line + 1) else { continue };
        if ti > 0 {
            o.push('\n');
        }
        let head: Vec<String> = source_cells(lines, h).into_iter().map(|x| super::strip_alias(&x.replacen("->", "→", 1))).collect();
        let rows: Vec<Vec<String>> = ch
            .rows
            .iter()
            .map(|r| {
                let mut cells: Vec<String> = source_cells(lines, r.span.line).iter().map(|x| super::customer_cell(x)).collect();
                if let (Some(last), Some(ChanceTest::Is(p))) = (cells.last_mut(), &r.chance) {
                    *last = tr!("{}（{}）", "{} ({})", last, percent_text(*p));
                }
                cells
            })
            .collect();
        o.push_str(&md_table(&head, &rows, false));
    }
    o
}

// ---------------------------------------------------------------------------------------------
// The HTML page
// ---------------------------------------------------------------------------------------------

/// The key the page's `FN` gets for the generated function that counts the answers, for a rule with
/// random inputs: `, chances: cacheFindChances`.
pub(super) fn fn_entry(f: &RuleFile) -> String {
    if !f.has_random() {
        return String::new();
    }
    format!(", chances: {}", crate::codegen::chances_fn_name(&crate::codegen::pub_name_of(&f.name), "javascript"))
}

/// The words the panel shows over the chances of the case entered.
pub(super) fn page_text(f: &RuleFile) -> Option<(String, String)> {
    if !f.has_random() {
        return None;
    }
    let names: Vec<String> = f.random_inputs().map(|i| i.name.text.clone()).collect();
    Some((
        tr!("{} を無作為に引いたときの、答えごとの確率", "The chance of each answer, with {} drawn at random", names.join(&tr!("・", ", "))),
        tr!("確率", "chance"),
    ))
}

/// The panel of a rule with random inputs: a place for the chances of the case entered, under the
/// result.
pub(super) fn panel(p: String) -> String {
    const RESULT: &str = "<div id=\"try-result\"></div>\n";
    debug_assert!(p.contains(RESULT), "the panel no longer has its result");
    p.replacen(RESULT, &format!("{RESULT}<div id=\"try-chances\"></div>\n"), 1)
}

/// Where the panel's script calls for the chances of the case entered: in `run`, once the inputs
/// are read, whether or not the case itself has an answer.
const RUN_AT: &str = "  for (const el of document.querySelectorAll(\".hit\")) el.classList.remove(\"hit\");\n";

/// The panel's script for a rule with random inputs: [`super::PAGE_JS`] (as the page has it) calling
/// for the chances in `run`, and the function that shows them.
pub(super) fn page_js(js: String) -> String {
    debug_assert!(js.contains(RUN_AT), "the page script no longer clears the lit rows in `run`");
    let mut js = js.replacen(RUN_AT, &format!("  rulecChances(args);\n{RUN_AT}"), 1);
    js.push_str(PAGE_CHANCES_JS);
    js
}

/// The chances of the case entered (§15.225): the inputs that are not random, as the fields hold
/// them, given to the generated function that counts the answers over every draw of the random ones,
/// and each answer shown with its count over the total as a fraction and a percentage, as the page's
/// `chances` table writes them. A function declaration, so that `run` can call it before this part
/// of the script has been reached; it reads nothing declared below the page script's first run.
const PAGE_CHANCES_JS: &str = r##"
function rulecChances(args) {
  const box = $("#try-chances");
  if (!box) return;
  box.textContent = "";
  const fixed = args.filter((_, i) => !(i < RULE.inputs.length && RULE.inputs[i].random));
  let ch;
  try {
    ch = FN.chances(...fixed);
  } catch (e) {
    box.textContent = e.message;
    return;
  }
  const gcd = (a, b) => (b === 0n ? a : gcd(b, a % b));
  const ratio = (n, total) => {
    const g = gcd(n, total);
    const q = (n * 2000n + total) / (2n * total);
    return String(n / g) + (total / g === 1n ? "" : "/" + String(total / g)) + " (" + String(q / 10n) + "." + String(q % 10n) + "%)";
  };
  const caption = document.createElement("div");
  caption.textContent = RULE.chances.caption;
  box.appendChild(caption);
  const table = document.createElement("table");
  const head = document.createElement("tr");
  for (const o of RULE.outputs) {
    const th = document.createElement("th");
    th.textContent = o.name;
    head.appendChild(th);
  }
  const th = document.createElement("th");
  th.textContent = RULE.chances.chance;
  head.appendChild(th);
  table.appendChild(head);
  for (const [out, n] of ch.ways) {
    const tr = document.createElement("tr");
    const vals = RULE.outputs.length === 1 ? [out] : RULE.outputs.map((o) => out[o.alias]);
    RULE.outputs.forEach((o, i) => {
      const td = document.createElement("td");
      td.textContent = vals[i] === null || vals[i] === undefined ? "-" : shown(o, vals[i]);
      tr.appendChild(td);
    });
    const td = document.createElement("td");
    td.textContent = ratio(n, ch.total);
    tr.appendChild(td);
    table.appendChild(tr);
  }
  box.appendChild(table);
}
"##;
