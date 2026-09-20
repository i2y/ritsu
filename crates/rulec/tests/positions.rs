//! Every position a value can sit in, and whether it is held to the type of the thing it
//! sits in.
//!
//! This file exists because "green" stopped meaning "checked". The corpus is made of correct
//! rules, so a check that never runs looks exactly like a check that passes, and three
//! positions were found this way in one afternoon (DESIGN §15.85, §15.86):
//!
//! - the cells of `examples` went through no type check at all, so an expected `800kg` under
//!   a `money[円]` output was compared by skipping the comparison, and the row passed;
//! - a rounding grid in the wrong unit fell back on a grid of one unit, so `round up(10銭)`
//!   and `round up(10)` both rounded to 1円, ten times from either reading;
//! - a `fold` answering `empty -> 999銭` under a `money[円]` output generated 99900円.
//!
//! All three are one bug: a literal met a declared type, and nobody compared them. So the
//! test is not "these three are fixed" but **the whole table of positions**, each seeded the
//! way `tests/mutants` seeds a corpus rule — the rule is checked clean first, then one
//! substitution puts a value of the wrong unit in one position, and the code has to appear.
//! A position added to the language without a row here is a position nothing is holding.

fn codes(src: &str) -> Vec<String> {
    rulec::check_source(src, "positions.rule").iter().map(|d| d.code.to_string()).collect()
}

/// The discipline of `tests/mutants`, inline: the rule is clean, one substitution breaks one
/// position, and that has to be said. `from` must occur exactly once, or the seed is not the
/// single change it claims to be.
#[track_caller]
fn seed(what: &str, good: &str, from: &str, to: &str, want: &str) {
    let clean = codes(good);
    assert!(clean.is_empty(), "{what}: 土台の規則がすでに汚れている: {clean:?}\n{good}");
    assert_eq!(good.matches(from).count(), 1, "{what}: `{from}` が一度だけ現れていない");
    let broken = good.replace(from, to);
    let got = codes(&broken);
    assert!(
        got.contains(&want.to_string()),
        "{what}: `{from}` を `{to}` にしても {want} が出ない（出たのは {got:?}）\n{broken}"
    );
}

/// Inputs, outputs, a derive, a define, a table, a clause, a result and examples.
const PLAIN: &str = r#"rule 基本(plain) v1

enum 区分(kind) = 甲(a) | 乙(b)
group 前半(first_half) = 甲

inputs
  重さ(w)  : mass[g]         range >=0g <=1000g
  割合(p)  : rate[step 1%]   range >=0% <=100%
  種別(k)  : 区分

outputs
  料金(fee) : money[円, incl_tax]  round up(10円)

derive 増量(more) : mass[g] = 重さ + 100g  range >=100g <=1100g
define 重い(heavy) : bool = 増量 >= 500g

table 判定(judge)
policy unique
| 重い  | 種別 | -> 基本額(base) : money[円, incl_tax] |
| true  | 前半 | 800円                                 |
| true  | 乙   | 900円                                 |
| false | -    | 500円                                 |

clause 特例(special) -> 基本額
  when 割合 >=90%
  then 100円
  overrides 判定

result 料金 = 基本額 + 10円

examples
| 重さ  | 割合 | 種別 | -> 料金 |
| 100g  | 10%  | 甲   | 510円   |
| 600g  | 10%  | 甲   | 810円   |
"#;

/// A walk: elements, a per-element table, a fold with all four kinds of arm, sequences.
const WALK: &str = r#"rule 畳み(walk) v1

enum 採用(verdict) = 取る(take) | 送る(skip)

elements 行(rows)
  額(amount) : money[円, incl_tax]  range >=0円 <=10000円
  率(share)  : rate[step 1%]        range >=0% <=100%

outputs
  合計(total) : money[円, incl_tax]  round up(1円)

table 行判定(row_of)
policy unique
| 額       | 率    | -> 採用(verdict) : 採用 |
| >=1000円 | -     | 取る                    |
| <1000円  | -     | 送る                    |

fold 採用 over 行
  送る      -> next
  取る      -> keep_max 額 by 額
  empty     -> 0円
  exhausted -> held

sequence 一件(one)
| 額      | 率   |
| 2000円  | 10%  |

sequence 空(none)
| 額 | 率 |

examples
| 行     | -> 合計 |
| 一件   | 2000円  |
| 空     | 0円     |
"#;

/// A count: the other ending of a walk, with its own range.
const COUNT: &str = r#"rule 数え(tally) v1

inputs
  可否(ok) : bool

elements 候補(cands)
  重さ(w) : mass[g]  range >=0g <=1000g

outputs
  手続き(action) : bool

table 一件(row_of)
policy unique
| 重さ    | -> 該当(hit) : bool |
| >=500g  | true                |
| <500g   | false               |

count 件数(n) over 候補 where 該当 = true  range >=0 <=50

table 判定(judge)
policy unique
| 件数 | 可否 | -> 手続き(action) : bool |
| 0    | -    | false                    |
| >=1  | -    | true                     |

sequence 一件だけ(one)
| 重さ  |
| 600g  |

examples
| 可否 | 候補   | -> 手続き |
| true | 一件だけ | true    |
"#;

/// A literal of the wrong unit, in every position a literal can be written.
#[test]
fn 単位の違うリテラルはどの位置でも止まる() {
    // --- Declarations
    seed("入力の range の下端", PLAIN, "range >=0g <=1000g", "range >=0円 <=1000g", "E103");
    seed("入力の range の上端", PLAIN, "range >=0g <=1000g", "range >=0g <=1000円", "E103");
    seed("導出の range", PLAIN, "range >=100g <=1100g", "range >=100円 <=1100円", "E103");
    seed("出力の丸めの格子", PLAIN, "round up(10円)", "round up(10g)", "E103");
    seed("出力の丸めの格子（単位なし）", PLAIN, "round up(10円)", "round up(10)", "E103");
    seed("出力の丸めの格子（補助単位）", PLAIN, "round up(10円)", "round up(10銭)", "E103");
    seed("型の刻み", PLAIN, "rate[step 1%]", "rate[step 1g]", "E103");
    seed("要素の欄の刻み", WALK, "rate[step 1%]", "rate[step 1円]", "E103");
    seed("要素の欄の range", COUNT, "range >=0g <=1000g", "range >=0g <=1000円", "E103");

    // --- Expressions
    seed("導出の式", PLAIN, "= 重さ + 100g", "= 重さ + 100円", "E103");
    seed("定義の式", PLAIN, ">= 500g", ">= 500円", "E103");
    seed("result の式", PLAIN, "+ 10円", "+ 10g", "E103");

    // --- Table cells
    seed("表の入力セル", PLAIN, "| false | -    | 500円", "| <500円 | -    | 500円", "E103");
    seed("表の出力セル", PLAIN, "| 800円 ", "| 800g  ", "E103");
    seed("節の when", PLAIN, "when 割合 >=90%", "when 割合 >=90g", "E103");
    seed("節の then", PLAIN, "then 100円", "then 100g", "E103");

    // --- Examples: the block that had no check of its own at all
    seed("例の入力セル", PLAIN, "| 100g  | 10%", "| 100円 | 10%", "E103");
    seed("例の期待値", PLAIN, "| 510円   |", "| 510g    |", "E103");
    seed("例の期待値（単位なし）", PLAIN, "| 510円   |", "| 510     |", "E103");
    seed("例の期待値（補助単位）", PLAIN, "| 510円   |", "| 999銭   |", "E103");

    // --- The walk
    seed("畳み込みの empty", WALK, "empty     -> 0円", "empty     -> 0g", "E103");
    seed("畳み込みの empty（補助単位）", WALK, "empty     -> 0円", "empty     -> 999銭", "E103");
    seed("並びのセル", WALK, "| 2000円  | 10%  |", "| 2000g   | 10%  |", "E103");
    seed("count の range", COUNT, "range >=0 <=50", "range >=0g <=50g", "E030");
}

/// The same table, for a value that is a **name** rather than a literal: an enum value that
/// is not one. A group's members went unchecked, so a group of 47 prefectures with one
/// misspelling was a group of 46 and said nothing (§15.86).
#[test]
fn 存在しない値はどの位置でも止まる() {
    seed("群の一員", PLAIN, "group 前半(first_half) = 甲", "group 前半(first_half) = 甲, 丙", "E012");
    seed("表の入力セル", PLAIN, "| true  | 乙   |", "| true  | 丙   |", "E012");
    seed("例の入力セル", PLAIN, "| 100g  | 10%  | 甲   |", "| 100g  | 10%  | 丙   |", "E012");
    seed("畳み込みの腕", WALK, "  送る      -> next", "  丙        -> next", "E024");
    seed("count の where", COUNT, "where 該当 = true", "where 該当 = 丙", "E029");
}

/// A column heading that names nothing. The examples' headings were read by nobody, so one
/// that named nothing was ignored outright and every cell under it went with it.
#[test]
fn 宣言されていない列はどの表でも止まる() {
    seed("表の見出し", PLAIN, "| 重い  | 種別 |", "| 幻列  | 種別 |", "E012");
    seed("例の見出し（入力）", PLAIN, "| 重さ  | 割合 | 種別 | -> 料金 |", "| 幻列  | 割合 | 種別 | -> 料金 |", "E012");
    seed("例の見出し（出力）", PLAIN, "-> 料金 |", "-> 幻列 |", "E012");
    seed("節の when の列", PLAIN, "when 割合", "when 幻列", "E012");
}

/// The other axis: not "the wrong unit" but **the wrong kind of value**. A date, a string, a
/// boolean, a value that is not on the column's step.
///
/// The unit axis above is not enough on its own. Only `Lit::Num` was ever read in an output
/// cell, so `2026-04-01` in a `money[円]` column generated the day number — **20544円** — and
/// a string generated 0, both silently (§15.88). The same literal in an *input* cell fell off
/// the end of the match and was reported as E102, "unreachable row", which sends the reader
/// to look at every other row.
#[test]
fn 種類の違う値はどの位置でも止まる() {
    // --- A date where a date is not wanted
    seed("表の入力セル", PLAIN, "| false | -    | 500円", "| 2026-04-01 | - | 500円", "E103");
    seed("表の出力セル", PLAIN, "| 800円 ", "| 2026-04-01 ", "E103");
    seed("例の入力セル", PLAIN, "| 100g  | 10%", "| 2026-04-01 | 10%", "E103");
    seed("例の期待値", PLAIN, "| 510円   |", "| 2026-04-01 |", "E103");
    seed("節の then", PLAIN, "then 100円", "then 2026-04-01", "E103");
    seed("畳み込みの empty", WALK, "empty     -> 0円", "empty     -> 2026-04-01", "E103");
    seed("並びのセル", WALK, "| 2000円  | 10%  |", "| 2026-04-01 | 10% |", "E103");

    // --- A string where a string is not wanted
    seed("表の入力セル（文字列）", PLAIN, "| false | -    | 500円", "| \"重い\" | - | 500円", "E103");
    seed("表の出力セル（文字列）", PLAIN, "| 800円 ", "| \"800\" ", "E103");
    seed("例の期待値（文字列）", PLAIN, "| 510円   |", "| \"510円\" |", "E103");
    seed("畳み込みの empty（文字列）", WALK, "empty     -> 0円", "empty     -> \"ゼロ\"", "E103");

    // --- A value that is not on the column's declared step
    seed("表の入力セル（刻み）", PLAIN, "when 割合 >=90%", "when 割合 >=90.5%", "E114");
    seed("例の入力セル（刻み）", PLAIN, "| 100g  | 10%", "| 100g  | 10.5%", "E114");
    seed("要素の欄（刻み）", WALK, "| 2000円  | 10%  |", "| 2000円  | 10.5% |", "E114");

    // --- A boolean where an enum is wanted
    seed("表の入力セル（真偽）", PLAIN, "| true  | 乙   |", "| true  | true |", "E103");
    seed("例の入力セル（真偽）", PLAIN, "| 100g  | 10%  | 甲   |", "| 100g  | 10%  | true |", "E103");
}

/// A `none` cell in a column that has no absent value. It is refused, but by the completeness
/// machinery rather than by name: the row can never match, so E102 says the row is dead. That
/// is true and is not the reason, which is that the column is not optional.
#[test]
fn 必須の列の_none_は行が死ぬこととして出る() {
    seed("表の入力セル", PLAIN, "| true  | 乙   |", "| true  | none |", "E102");
}
