//! What is drawn, as the type check sees it (§15.224–§15.226): the inputs marked `random`, a fold
//! that picks by weight, and the shape of the `chances` tables. How likely each answer is, and
//! whether a claim holds, is `crate::chances`'s question; this is whether the rule says it in a way
//! that can be counted.

use super::{Checked, SymKind, Ty};
use crate::ast::*;
use crate::diag::{Diag, Span};

/// How many points the function that counts the answers may evaluate per call before W131 says
/// so (§15.225): the same million `diff` and the machine's walk stop at by default.
pub const FUNCTION_POINTS: u128 = 1_000_000;

/// The most points the random inputs may have together (§15.224): a count of ways has to fit in an
/// int64 in every language, and the ways are counted over every point.
pub const MOST_POINTS: u128 = 1 << 62;

impl Checked {
    /// The inputs marked `random` (§15.224): each one is drawn from something finite — a number
    /// with both ends of its range, an enum, a truth value — and is an input the caller passes once
    /// per call. E070 says what cannot be drawn; E108 says when the points are too many to count in
    /// an int64; W131 says when the function that counts the answers runs the rule over more than a
    /// million points on every call.
    pub(super) fn random_marks(&mut self, f: &RuleFile, path: &str) {
        let at = |line: usize| format!("{path}:{line}");
        let carried: Vec<&str> = f
            .machine
            .iter()
            .flat_map(|m| m.carry.iter().chain(m.carries.iter()).map(|(i, _, _)| i.text.as_str()))
            .collect();
        let mut drawable = true;
        for i in &f.inputs {
            let Some(sp) = &i.random else { continue };
            let bad = |s: &mut Self, mark: String, note: String| {
                s.diags.push(
                    Diag::error("E070", tr!("入力 {} は乱数として引けません", "Input {} cannot be drawn", i.name.text))
                        .at(at(sp.line))
                        .mark(sp.clone(), mark)
                        .note(note)
                        .note(tr!(
                            "乱数として引けるのは、範囲の両端を書いた数（金額・量・率も、刻みの整数倍ずつ）、列挙、真偽です。",
                            "What can be drawn is a number with both ends of its range written (an amount, a quantity or a rate too, in whole steps), an enum, or a truth value."
                        )),
                );
            };
            let ty = self.syms.get(&i.name.text).map(|s| s.ty.clone()).unwrap_or(Ty::Unknown);
            if carried.contains(&i.name.text.as_str()) {
                bad(
                    self,
                    tr!("{} は前の呼び出しの答えを持ち越す入力です", "{} is carried over from the call before", i.name.text),
                    tr!(
                        "持ち越す入力は、前の呼び出しが答えた値です。呼ぶ側が引く値ではありません。",
                        "A carried input is what the call before answered, not a value the caller draws."
                    ),
                );
                drawable = false;
                continue;
            }
            if i.from.is_some() {
                bad(
                    self,
                    tr!("{} は呼ぶ側のオブジェクトから取り出す入力です", "{} is taken out of the caller's object", i.name.text),
                    tr!(
                        "`from` が呼ぶ側のオブジェクトから取り出す値は、呼ぶ側の値で、引いた値ではありません。乱数の入力は、呼ぶ側が引いてそのまま渡す値です。引くのなら `from` を、取り出すのなら `random` を外してください。",
                        "What `from` reads out of the caller's object is the caller's value, not a draw; a random input is a value the caller draws and passes as it is. Take the `from` off the line if the value is drawn, or the `random` if it is read."
                    ),
                );
                drawable = false;
                continue;
            }
            let why = match &ty {
                Ty::Opt(_) => Some(tr!(
                    "値の無いことがある入力（`?`）は、引いた値が無いことの見込みが決まりません。",
                    "An input that may be absent (`?`) leaves the chance of being absent undecided."
                )),
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => match self.ranges.get(&i.name.text) {
                    Some((Some(_), Some(_))) => None,
                    _ => Some(tr!(
                        "範囲の両端が無いと、引く値の数が決まりません。`range >=0 <=99` のように両端を書いてください。",
                        "Without both ends of its range, the number of values to draw from is not known. Write both, as in `range >=0 <=99`."
                    )),
                },
                Ty::Enum(_) | Ty::Bool => None,
                Ty::Unknown => None,
                other => Some(tr!(
                    "{other} は数えられる点の並びにならないので、一様に引けません。",
                    "{other} does not come as countable points, so it cannot be drawn uniformly."
                )),
            };
            if let Some(note) = why {
                bad(self, tr!("{} は {} です", "{} is {}", i.name.text, ty), note);
                drawable = false;
            }
        }
        for fd in f.element_fields() {
            if let Some(sp) = &fd.random {
                self.diags.push(
                    Diag::error("E070", tr!("要素のフィールド {} は乱数にできません", "Element field {} cannot be random", fd.name.text))
                        .at(at(sp.line))
                        .mark(sp.clone(), "")
                        .note(tr!(
                            "乱数は、一回の呼び出しに一つ引く値です。要素のフィールドは要素ごとに呼ぶ側が渡す値で、要素の数だけ引くことになり、点の数が並びの長さで変わります。重みで選ぶなら、`fold` の腕 `take_weighted` と、乱数の入力を指す `draw` の行を使ってください。",
                            "A random value is drawn once per call. A field of an element is passed once per element, so it would be drawn as many times as there are elements, and the number of points would turn on the length of the sequence. To pick by weight, use the fold arm `take_weighted` with a `draw` line naming a random input."
                        )),
                );
            }
        }
        if !drawable || !f.has_random() {
            return;
        }
        // The points of every random input together: each count of ways is at most this.
        let mut total: u128 = 1;
        for i in f.random_inputs() {
            match crate::chances::points_of(self, &i.name.text) {
                Some(n) => total = total.saturating_mul(n),
                None => return,
            }
        }
        let first = f.random_inputs().next().and_then(|i| i.random.clone()).unwrap_or_else(|| Span::new(1, 0, 1));
        if total > MOST_POINTS {
            let names = f.random_inputs().map(|i| i.name.text.clone()).collect::<Vec<_>>().join(", ");
            self.diags.push(
                Diag::error("E108", tr!("乱数の入力の点の数が int64 に収まりません", "The points of the random inputs do not fit in int64"))
                    .at(at(first.line))
                    .mark(first.clone(), tr!("{names} の点は合わせて 2^62 を越えます", "{names} have more than 2^62 points together"))
                    .note(tr!(
                        "確率は、乱数の入力の点を数えて出します。数えた通りの数はどの言語でも int64 で持つので、点の数は 2^62 までです。乱数の入力の範囲を狭めてください。",
                        "A chance is worked out by counting the points of the random inputs, and every language holds the count in an int64, so there can be at most 2^62 of them. Narrow the ranges of the random inputs."
                    )),
            );
            return;
        }
        if matches!(crate::chances::method(f), crate::chances::Method::Points) && total > FUNCTION_POINTS {
            let name = crate::codegen::chances_fn_name(&f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone()), "python");
            self.diags.push(
                Diag::warning("W131", tr!("確率の関数は、呼ぶたびに規則を {total} 回評価します", "The chances function runs the rule {total} times on every call"))
                    .at(at(first.line))
                    .mark(first.clone(), tr!("乱数の入力の点は合わせて {total} です", "the random inputs have {total} points together"))
                    .note(tr!(
                        "生成する確率の関数（Python なら `{name}`）は、乱数の入力の点を全部回して規則を呼び、答えごとに数えます。正確で、規則の関数と一行も食い違いませんが、一回の呼び出しの手間は点の数です。",
                        "The function that counts the answers (`{name}` in Python) runs the rule over every point of the random inputs and counts each answer. It is exact and never disagrees with the rule's function, but one call costs as many runs as there are points."
                    ))
                    .note(tr!(
                        "頻繁に呼ぶなら、乱数の入力の範囲を狭めてください。重みで選ぶ規則なら、乱数の入力を `draw` の行だけが読むようにすると、関数は点を回さずに重みから数えます。",
                        "If it is called often, narrow the ranges of the random inputs. In a rule that picks by weight, let only the `draw` line read the random input, and the function counts from the weights without running the points."
                    )),
            );
        }
    }

    /// A fold that picks by weight (§15.226): its arms are `next` and `take_weighted` alone, its
    /// weights are numbers that cannot be negative, and its `draw` line names a random input — a
    /// number whose range starts at 0 — that the running total of the weights is compared with. One
    /// such fold to a rule. E073 for anything else.
    pub(super) fn weighted_fold(&mut self, fold: &FoldDecl, f: &RuleFile, at: &str, path: &str) {
        let e073 = |s: &mut Self, title: String, sp: &Span, mark: String, note: String| {
            s.diags.push(Diag::error("E073", title).at(at.to_string()).mark(sp.clone(), mark).note(note));
        };
        if !fold.weighted() {
            if let Some(d) = &fold.draw {
                e073(
                    self,
                    tr!("`draw` の行がありますが、重みで選ぶ腕がありません", "There is a `draw` line, and no arm that picks by weight"),
                    &d.span,
                    tr!("{} を比べる `take_weighted` がありません", "no `take_weighted` compares {}", d.text),
                    tr!(
                        "`draw -> <乱数の入力>` は、`take_weighted` の腕が重みの合計と比べる値を言う行です。重みで選ばないなら消してください。",
                        "`draw -> <random input>` names the value a `take_weighted` arm compares the running total of the weights with. Take it out if nothing picks by weight."
                    ),
                );
            }
            return;
        }
        // One pick by weight to a rule: the count of ways is then the weights', with no second
        // pick drawing on the same value.
        let first = f.folds().find(|x| x.weighted()).map(|x| x.span.line) == Some(fold.span.line);
        if !first {
            e073(
                self,
                tr!("重みで選ぶ畳み込みが二つあります", "There are two folds that pick by weight"),
                &fold.span,
                tr!("二つ目です", "this is the second"),
                tr!(
                    "一つの規則で重みで選ぶのは一回です。二つの選び方が要るなら、規則を二つに分けてください。",
                    "A rule picks by weight once. Split the rule in two if it needs two picks."
                ),
            );
        }
        for (n, arm, sp) in &fold.arms {
            if !matches!(arm, Arm::Next | Arm::TakeWeighted { .. }) {
                e073(
                    self,
                    tr!("重みで選ぶ畳み込みに、ほかの腕があります: {}", "A fold that picks by weight has another kind of arm: {}", n.text),
                    sp,
                    tr!("書けるのは `next` と `take_weighted` だけです", "only `next` and `take_weighted` can be written here"),
                    tr!(
                        "重みで選ぶときは、どの要素の重みも合計に入ってから一つに決まります。途中で止めたり別の値を採ったりすると、重みの合計と選ばれる見込みの関係が崩れます。",
                        "A pick by weight is settled once every element's weight is in the total. Stopping on the way or taking another value would break the tie between the weights and the chance of each being picked."
                    ),
                );
            }
        }
        // The weights: whole numbers that cannot go below zero.
        for (n, arm, _) in &fold.arms {
            let Arm::TakeWeighted { weight, .. } = arm else { continue };
            let ty = self.expr_ty(weight, path);
            if !matches!(ty.present(), Ty::Number | Ty::Unknown) {
                e073(
                    self,
                    tr!("重みは number です（{} の腕に {ty} が書かれています）", "A weight is a number (the arm of {} gives {ty})", n.text),
                    weight.span(),
                    "".into(),
                    tr!(
                        "重みは、選ばれる見込みの割合を決める整数です。金額や率をそのまま使うなら、`down(額 / 1円, 1)` のように整数にしてください。",
                        "A weight is a whole number that sets the share of the chance. To use an amount or a rate, make it a whole number first, as in `down(amount / 1USD, 1)`."
                    ),
                );
                continue;
            }
            match self.interval(weight, &Ty::Number) {
                Some((lo, _)) if lo.num >= 0 => {}
                iv => {
                    let shown = iv.map(|(lo, hi)| format!("{lo}..{hi}")).unwrap_or_else(|| tr!("決まりません", "unknown"));
                    e073(
                        self,
                        tr!("重みが負になりえます", "A weight can be negative"),
                        weight.span(),
                        tr!("重みの範囲は {shown} です", "the weight's range is {shown}"),
                        tr!(
                            "負の重みには見込みがありません。重みが読む値の範囲を 0 以上にするか、`max(重み, 0)` で書いてください。",
                            "A negative weight has no chance to stand for. Make the values the weight reads 0 or more, or write `max(weight, 0)`."
                        ),
                    );
                }
            }
        }
        let Some(d) = &fold.draw else {
            e073(
                self,
                tr!("重みで選ぶ畳み込みに `draw` の行がありません", "A fold that picks by weight has no `draw` line"),
                &fold.span,
                tr!("`draw -> <乱数の入力>` がありません", "there is no `draw -> <random input>`"),
                tr!(
                    "`take_weighted` は、要素の重みを順に足していき、引いた値がその合計を初めて下回った要素を取ります。比べる乱数を `draw -> <乱数の入力>` で書いてください。",
                    "`take_weighted` adds up the weights as the walk goes, and takes the element at which the drawn value is first below the total. Name the drawn value with `draw -> <random input>`."
                ),
            );
            return;
        };
        self.used.insert(d.text.clone());
        let sym = self.syms.get(&d.text).cloned();
        let why = match &sym {
            None => Some((tr!("{} は宣言されていません", "{} is not declared", d.text), tr!("比べる乱数は、`random` を付けた入力です。", "The drawn value is an input marked `random`."))),
            Some(s) if s.kind != SymKind::Input => Some((
                tr!("{} は入力ではありません", "{} is not an input", d.text),
                tr!("比べる乱数は、呼ぶ側が引いて渡す入力です。`inputs` に `random` を付けて宣言してください。", "The drawn value is an input the caller draws and passes. Declare it under `inputs`, marked `random`."),
            )),
            Some(_) if !f.is_random(&d.text) => Some((
                tr!("{} は乱数の入力ではありません", "{} is not a random input", d.text),
                tr!(
                    "比べる値が一様に引かれていないと、重みの割合が選ばれる見込みになりません。宣言の行の最後に `random` を付けてください。",
                    "Unless the value is drawn uniformly, the share of the weights is not the chance of being picked. Mark the declaration `random` at the end of its line."
                ),
            )),
            Some(s) if !matches!(s.ty, Ty::Number) => Some((
                tr!("{} は {} です", "{} is {}", d.text, s.ty),
                tr!("比べる乱数は、重みの合計と同じ整数の number です。", "The drawn value is a whole number, as the total of the weights is."),
            )),
            Some(_) => match self.ranges.get(&d.text) {
                Some((Some(lo), Some(_))) if lo.num == 0 => None,
                _ => Some((
                    tr!("{} の範囲が 0 から始まっていません", "the range of {} does not start at 0", d.text),
                    tr!(
                        "重みの合計は 0 から数えるので、比べる乱数の範囲も `range >=0 <=<上限>` にします。上限は、重みの合計がとりうる値より下にならないようにしてください。",
                        "The total of the weights counts from 0, so the drawn value's range is `range >=0 <=<top>`, with the top no lower than the total of the weights can be."
                    ),
                )),
            },
        };
        if let Some((mark, note)) = why {
            e073(self, tr!("`draw` が指す値が、重みで選ぶ乱数になりません", "What `draw` names cannot be the value a pick by weight draws"), &d.span, mark, note);
        }
    }

    /// The `chances` tables (§15.224): every input that is not random has a column, and no random
    /// one has; for a rule that walks, every sequence it walks has one, its cells naming a
    /// `sequence`; the columns after `->` are outputs; every cell fits its column. E070 for the
    /// shape; the cells are held to their columns' types as a table's are.
    pub(super) fn chances_tables(&mut self, f: &RuleFile, path: &str) {
        if f.chances.is_empty() {
            return;
        }
        let at = |line: usize| tr!("{path}:{line} 確率の主張", "{path}:{line} chances");
        if !f.has_random() {
            let sp = f.chances[0].span.clone();
            self.diags.push(
                Diag::error("E070", tr!("乱数の入力が無いのに `chances` があります", "There is a `chances` table and no random input"))
                    .at(at(sp.line))
                    .mark(sp, "")
                    .note(tr!(
                        "確率は、`random` を付けた入力を一様に引いたときの答えの見込みです。どの入力も引かないなら、答えは入力で一つに決まり、`examples` で書けます。",
                        "A chance is how likely an answer is when the inputs marked `random` are drawn uniformly. With nothing drawn, the inputs settle the answer, and `examples` say it."
                    )),
            );
            return;
        }
        // W111 asks whether a **table** uses a declaration; a claim is not a table, as an example is
        // not, so what typing the cells marks as used is put back afterwards.
        let used = self.used.clone();
        let used_values = self.used_values.clone();
        let seqs: Vec<String> = f
            .elements
            .iter()
            .filter(|el| f.items.iter().any(|it| matches!(it, Item::Agg(d) if d.over == el.name.text) || matches!(it, Item::Fold(d) if d.over == el.name.text)))
            .map(|el| el.name.text.clone())
            .collect();
        for ch in &f.chances {
            let mut seen: Vec<&str> = Vec::new();
            let bad_col = |s: &mut Self, name: &str, sp: &Span, title: String, note: String| {
                s.diags.push(Diag::error("E070", title).at(at(sp.line)).mark(sp.clone(), name.to_string()).note(note));
            };
            for (n, sp) in ch.inputs.iter().chain(ch.outputs.iter()) {
                if seen.contains(&n.as_str()) {
                    bad_col(self, "", sp, tr!("列 {n} が二つあります", "There are two columns {n}"), tr!("一つの名前に一つの列です。", "One column per name."));
                }
                seen.push(n);
            }
            for (n, sp) in &ch.inputs {
                if seqs.contains(n) {
                    continue;
                }
                if f.is_random(n) {
                    bad_col(
                        self,
                        "",
                        sp,
                        tr!("{n} は乱数の入力なので、列にできません", "{n} is a random input and cannot be a column"),
                        tr!(
                            "主張は、乱数の入力を一様に引いたときの見込みです。引く値は列に書かず、規則がその全部の点を数えます。",
                            "A claim is about drawing the random inputs uniformly. What is drawn has no column: the check counts every one of its points."
                        ),
                    );
                    continue;
                }
                if !f.inputs.iter().any(|i| i.name.text == *n) {
                    let what = if f.outputs.iter().any(|o| o.name.text == *n) {
                        tr!("{n} は出力です。出力の列は `->` の右に書いてください", "{n} is an output: write the output columns right of `->`")
                    } else {
                        tr!("左の列に書けるのは、乱数でない入力と、たどる並びです", "the columns on the left are the inputs that are not random, and the sequences walked")
                    };
                    bad_col(self, "", sp, tr!("{n} は乱数でない入力ではありません", "{n} is not an input that is not random"), what);
                }
            }
            let missing: Vec<String> = f
                .inputs
                .iter()
                .filter(|i| i.random.is_none())
                .map(|i| i.name.text.clone())
                .chain(seqs.iter().cloned())
                .filter(|n| !ch.inputs.iter().any(|(c, _)| c == n))
                .collect();
            if !missing.is_empty() {
                let sp = ch.rows.first().map(|r| r.span.clone()).unwrap_or_else(|| ch.span.clone());
                self.diags.push(
                    Diag::error("E070", tr!("`chances` に列の無い入力があります: {}", "`chances` has no column for {}", missing.join(", ")))
                        .at(at(sp.line))
                        .mark(sp, tr!("見出しに足してください", "add them to the header"))
                        .note(tr!(
                            "主張は、乱数でない入力の組み合わせごとの見込みです。どの組み合わせについて言うのかを、乱数でない入力の全部の列で書きます。どの値でもよい列には `-` を書いてください。並びをたどる規則では、並びの列に `sequence` の名前を書きます。",
                            "A claim is about the chances for combinations of the inputs that are not random, so every one of them has a column saying which; a column that takes any value gets `-`. In a rule that walks, a sequence's column names a `sequence`."
                        )),
                );
            }
            for (n, sp) in &ch.outputs {
                if !f.outputs.iter().any(|o| o.name.text == *n) {
                    bad_col(
                        self,
                        "",
                        sp,
                        tr!("{n} は出力ではありません", "{n} is not an output"),
                        tr!("`->` の右に書けるのは、主張に要る出力だけです。", "The columns right of `->` are the outputs the claims are about."),
                    );
                }
            }
            // The cells, held to their columns as a table's are.
            for row in &ch.rows {
                let at_row = at(row.span.line);
                for (ci, cell) in row.cells.iter().enumerate() {
                    let Some((n, _)) = ch.inputs.get(ci) else { continue };
                    let sp = row.cell_spans.get(ci).cloned().unwrap_or_else(|| row.span.clone());
                    if seqs.contains(n) {
                        let ok = match cell {
                            Cell::Lit(Lit::Word(w)) => f
                                .sequences
                                .iter()
                                .find(|sq| sq.name.text == *w)
                                .and_then(|sq| super::sequence_list(f, sq))
                                .is_some_and(|k| f.elements[k].name.text == *n),
                            _ => false,
                        };
                        if !ok {
                            self.diags.push(
                                Diag::error("E070", tr!("この列には {n} の `sequence` の名前を書いてください", "This column holds the name of a `sequence` of {n}"))
                                    .at(at_row.clone())
                                    .mark(sp, "")
                                    .note(tr!(
                                        "並びをたどる規則の主張は、例と同じく、たどる並びを `sequence <名前>` で書いてその名前を指します。",
                                        "As in an example, a claim of a rule that walks names the sequence it is about, written with `sequence <name>`."
                                    )),
                            );
                        }
                        continue;
                    }
                    let Some(ty) = self.syms.get(n).map(|s| s.ty.clone()) else { continue };
                    let sc = *self.scales.get(n).unwrap_or(&1);
                    self.cell(cell, &ty, sc, &sp, &at_row, true);
                }
                for (oi, cell) in row.outs.iter().enumerate() {
                    let Some((n, _)) = ch.outputs.get(oi) else { continue };
                    let Some(ty) = self.syms.get(n).map(|s| s.ty.clone()) else { continue };
                    let sp = row.out_spans.get(oi).cloned().unwrap_or_else(|| row.span.clone());
                    let sc = *self.scales.get(n).unwrap_or(&1);
                    self.cell(cell, &ty, sc, &sp, &at_row, true);
                }
            }
        }
        self.used = used;
        self.used_values = used_values;
    }
}
