# 性質と証明

`examples` の一行は、一つの入力に対する答えを書きます。**性質**（property）は、関数の入力の全部について何かを言います。返金を配った分の合計は返金額になる、どの明細にも負の額は配らない、といったことです。kaname は、性質を自分で選んだ入力で試し、さらに Lean の定理の文として書き出します。入力の全部で成り立つことは、Lean での証明が示します。証明を探すのは kaname の仕事ではありません。人かエージェントが、`.kn` の隣のファイルに証明を書き、Lean がそれを確かめます。

## 性質を書く

例は `crates/kaname/examples/refund_split` です。注文の明細に、まだ払われていない残りの比で返金を配ります。`allocate` と `running_totals` を使います。

```kn
fn 按分する(split)(明細(lines): list[明細] count >=1 <=10, 返金(refund): money[USDc] range >=0USDc <=10_000_000USDc) -> list[明細の返金] total
  description "どの明細にも、その明細の終わりまでの返金から、手前までの返金を引いた分を配る"
  map(running_totals(明細, 一件 => 一件.残り), 途中 =>
    明細の返金 { 品番: 途中.item.品番, 残り: 途中.item.残り, 配分: allocate(返金, 途中.upto, 途中.total) - allocate(返金, 途中.before, 途中.total) })

property 配分の合計は返金額(shares_add_up)(明細(lines): list[明細] count >=1 <=10, 返金(refund): money[USDc] range >=0USDc <=10_000_000USDc)
  description "配った分を足すと返金額になる"
  sum(按分する(明細: 明細, 返金: 返金), 一件 => 一件.配分) = 返金
```

性質は関数と同じ形で書き、引数に範囲を宣言します。読み方は、その範囲のどの入力でも最後の行が `true` になる、です。`when` の行を書くと、性質が語る入力を絞れます。

```kn
property 残りより多くは配らない(no_share_is_more_than_its_left)(明細(lines): list[明細] count >=1 <=10, 返金(refund): money[USDc] range >=0USDc <=10_000_000USDc)
  description "残りの合計を超えない返金なら、どの明細にも、その明細の残りより多くは配らない"
  when 返金 <= sum(明細, 一件 => 一件.残り)
  all(按分する(明細: 明細, 返金: 返金), 一件 => 一件.配分 <= 一件.残り)
```

性質は呼べず、生成するコードにも出ません。`total` と書いた関数と同じに扱うので、失敗しうる本体は E401 です。失敗を「成り立たない」と読むか「当てはまらない」と読むかで、性質の意味が二つに割れるからです。引数に関数の型は書けません（E605）。

## `check` が言うこと

`kaname check` と `ritsu check` は、性質をいつも引数のベクタ（`kaname vectors` が選ぶ入力）で試します。規則と日付は、`examples` と同じく本物を呼びます。どれかのベクタで成り立たなければ、その入力を挙げてエラーにします。

```console
$ cd crates/kaname
$ ritsu kaname check tests/properties/shares.ja.kn --lang ja
エラー[E603]: tests/properties/shares.ja.kn:12:1: 性質 `どの明細にも配る` は、明細: [明細 { 品番: "a", 残り: 500000USDc }, 明細 { 品番: "a", 残り: 500000USDc }], 返金: 1USDc で成り立ちません
    12 | property どの明細にも配る(every_line_gets_a_share)(明細(lines): list[明細] count >=1 <=10, 返金(refund): money[USDc] range >=0USDc <=10_000_000USDc)
  = 15 行目: `配分` は [0USDc, 1USDc]
  = `check` は、性質を引数のベクタで参照インタプリタに流します。この入力では、`when` の行がどれも `true` で、最後の行が `false` になりました。
  = 関数を直すか、`when` の行か引数の範囲で性質を狭めるか、性質を直してください。
```

2 行の明細に 1 セントの返金を配ると、一つ目の明細に 0 セントが配られます。関数とこの性質のどちらが誤りかは、人が決めることです。

反例が無くても、性質を示したことにはなりません。ベクタは入力の一部だからです。`check` は警告で、示されていないことと、その理由を言います。`when` の行がどのベクタでも成り立たない性質も警告します。そういう性質は、最後の行が何であっても証明できてしまうからです。

```console
$ cd crates/kaname
$ ritsu kaname check tests/properties/thresholds.ja.kn --lang ja
警告[W603]: tests/properties/thresholds.ja.kn:4:1: 性質 `しきい値の先` は、`when` の行が成り立つ 3 個のベクタ（6 個のうち）のすべてで成り立ちましたが、示されていません。証明がありません（thresholds_ja.lean に、この性質の定理がありません）
     4 | property しきい値の先(past_the_threshold)(値(x): number range >=0 <=100)
  = `ritsu gen --target lean` が、定理の名前と `sorry` だけの下書き `samples/thresholds_ja.lean` を書きます。それを .kn の隣に thresholds_ja.lean として置き、`sorry` を証明に置き換えてください。
警告[W603]: tests/properties/thresholds.ja.kn:9:1: 性質 `範囲の外` は示されていません。証明がありません（thresholds_ja.lean に、この性質の定理がありません）
     9 | property 範囲の外(past_the_end)(値(x): number range >=0 <=100)
  = `ritsu gen --target lean` が、定理の名前と `sorry` だけの下書き `samples/thresholds_ja.lean` を書きます。それを .kn の隣に thresholds_ja.lean として置き、`sorry` を証明に置き換えてください。
警告[W605]: tests/properties/thresholds.ja.kn:9:1: 性質 `範囲の外` の `when` の行は、3 個のベクタのどれでも成り立ちません
     9 | property 範囲の外(past_the_end)(値(x): number range >=0 <=100)
  = どの入力でも `when` の行が成り立たない性質は、何を言っていても証明できてしまいます。`when` の行と引数の範囲を見直してください。
ok tests/properties/thresholds.ja.kn: 関数 0 個
```

性質の答えは三つです。

| 答え | いつ | `check` の表示 |
|---|---|---|
| 反例 | あるベクタで、`when` の行が成り立ち、最後の行が `false` になった | E603（入力つき） |
| 示した | `ritsu check --lean` が証明を受け付けた | 何も出ません |
| 決められない | 反例が無く、示してもいない | W603（か W604）と理由 |

W604 は、まだ Lean に書けないものを使う性質です。辞書、集合、`=` 以外の文字列の組み込み、`sort_by`・`distinct`・`max_by`・`min_by`・`to_dict`・`group_by`、キーに限った型の引数がそうです。ベクタでの確かめはします。規則や日付のファイルの答えに頼る性質も W603 で、理由はそれだと言います。証明が使えるのは、規則や日付の口が教える事実だけだからです。

## Lean での文

`ritsu gen --target lean` は、ほかの出力先と同じように、関数を Lean 4 の定義に、性質を定理の文に書き出します。

```console
$ cd crates/kaname/examples/refund_split
$ ritsu gen --root . --out generated --target lean
generated: generated/lean/lean-toolchain
generated: generated/lean/lakefile.toml
generated: generated/lean/.gitignore
generated: generated/lean/Kaname/Prelude.lean
generated: generated/lean/Kaname/refund_split_ja.lean
generated: generated/lean/Kaname/refund_split.lean
generated: generated/lean/samples/refund_split_ja.lean
generated: generated/lean/samples/refund_split.lean
```

`Kaname/refund_split.lean` に、定義と性質ごとの文が入ります。お金や量は、型の単位で数えた `Int` です。`Within` はレコードのフィールドの範囲を表します（以下は英語の版のモジュールの出力です。日本語の版は `Kaname.refund_split_ja` に書かれます）。

```lean
abbrev shares_add_up : Prop :=
  ∀ (lines : List Line) (refund : Int),
    1 ≤ lines.length → lines.length ≤ 10 → (∀ e₁ ∈ lines, e₁.Within) →
    0 ≤ refund → refund ≤ 10000000 →
    ((List.map (fun (s : Share) => s.share) (split lines refund)).sum) = refund
```

`samples/refund_split.lean` は下書きで、定理の名前だけが書いてあり、証明はどれも `sorry` です。

## 証明を書く

証明は、`.kn` の隣の `refund_split.lean` に書きます。下書きを最初の三行を除いて写し、`sorry` を証明に置き換えます。定理の名前は `Proofs.refund_split.<性質>` で、型は `ritsu gen` が書いた文そのものです。使うのは、Lean 自身の `omega`、`simp`、`decide`、`grind` と、丸め・`allocate`・`running_totals` のための `Kaname/Prelude.lean` の補題です。Mathlib は使いません。書きながら試すには、`generated/lean/` で `lake build` を走らせます。

例の証明のファイルの全体です。

```lean
import Kaname.refund_split

namespace Proofs.refund_split

open Kaname Kaname.refund_split

/-- What is left on one line or more, each of a cent or more, comes to more than nothing. -/
theorem lefts_pos : ∀ ls : List Line, 1 ≤ ls.length → (∀ l ∈ ls, l.Within) →
    0 < (ls.map fun l => l.left).sum
  | [], h, _ => by simp at h
  | l :: ls, _, h => by
    simp only [List.map_cons, List.sum_cons]
    have hl := (h l (List.mem_cons_self ..)).1
    have hs : 0 ≤ (ls.map fun l => l.left).sum := by
      match ls with
      | [] => simp
      | m :: ms => exact Int.le_of_lt (lefts_pos (m :: ms) (by simp) (fun x hx => h x (List.mem_cons_of_mem _ hx)))
    omega

/-- The shares, each the refund up to its line's end less the refund before it, telescope to the
refund of the whole. -/
theorem shares_add_up : Kaname.refund_split.shares_add_up := by
  intro lines refund hlen _ hwithin _ _
  have hT := lefts_pos lines hlen hwithin
  have key : (split lines refund).map (fun s => s.share)
      = (runningTotals lines (fun l => l.left)).map
          (fun r => allocate refund r.upto (lines.map fun l => l.left).sum - allocate refund r.before (lines.map fun l => l.left).sum) := by
    unfold split
    rw [List.map_map]
    apply List.map_congr_left
    intro r hr
    simp only [Function.comp, runningTotals_total _ _ r hr]
  rw [key, runningTotals_telescope lines (fun l => l.left) (fun x => allocate refund x (lines.map fun l => l.left).sum)]
  rw [allocate_self refund hT, allocate_zero]
  omega

/-- A share is the refund up to its line's end less the refund before it, and `allocate` does not go
down as its share grows. -/
theorem no_share_is_negative : Kaname.refund_split.no_share_is_negative := by
  intro lines refund hlen _ hwithin hr0 _ s hs
  have hT := lefts_pos lines hlen hwithin
  unfold split at hs
  rw [List.mem_map] at hs
  obtain ⟨r, hr, rfl⟩ := hs
  have hw : ∀ l ∈ lines, 0 ≤ (fun (l : Line) => l.left) l := fun l hl => by
    have := (hwithin l hl).1
    show 0 ≤ l.left
    omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hmono := allocate_mono (t := refund) hr0 (htot ▸ hT) (show r.before ≤ r.upto by
    have hitem : r.item ∈ lines := by
      have := List.mem_map_of_mem (f := (·.item)) hr
      rwa [runningTotals_items] at this
    have := (hwithin r.item hitem).1
    omega)
  show allocate refund r.upto r.total - allocate refund r.before r.total ≥ 0
  omega

/-- What one line gets is at most its width of the whole, when the refund is no more than the whole. -/
theorem no_share_is_more_than_its_left : Kaname.refund_split.no_share_is_more_than_its_left := by
  intro lines refund hlen _ hwithin hr0 _ hle s hs
  have hT := lefts_pos lines hlen hwithin
  unfold split at hs
  rw [List.mem_map] at hs
  obtain ⟨r, hr, rfl⟩ := hs
  have hw : ∀ l ∈ lines, 0 ≤ (fun (l : Line) => l.left) l := fun l hl => by
    have := (hwithin l hl).1
    show 0 ≤ l.left
    omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hitem : r.item ∈ lines := by
    have := List.mem_map_of_mem (f := (·.item)) hr
    rwa [runningTotals_items] at this
  have := (hwithin r.item hitem).1
  have hsub := allocate_sub_le (t := refund) hr0 (htot ▸ hle) (htot ▸ hT) (show r.before ≤ r.upto by omega)
  show allocate refund r.upto r.total - allocate refund r.before r.total ≤ r.item.left
  omega

end Proofs.refund_split
```

性質を示したと言うのは、`ritsu check --lean` だけです。`--lean` は、`.kn` から Lean のパッケージを作り直し（`generated/lean/` は読みません）、証明のファイルを文字として読んでから Lean にビルドさせ、証明をカーネルで流し直します。そのあと性質ごとに、定理があること、型が `ritsu gen` の書いた文であること、Lean の三つの公理（`propext`、`Classical.choice`、`Quot.sound`）のほかに頼らないことを確かめます。`sorry`、証明が自分で置いた公理、`native_decide` を使った証明は E604 です。

証明がすべて揃えば、`--lean` は W603 を出しません。

```console
$ cd crates/kaname
$ ritsu check --lean examples/refund_split --lang ja
ok examples/refund_split/refund_split.ja.kn: 関数 1 個、total
ok examples/refund_split/refund_split.kn: 関数 1 個、total
ritsu check: ファイル 2 個（kaname 2）。どれも検査を通りました。言語の境目: 確かめた 0 か所、決められない 0 か所
```

`no_share_is_negative` の証明を `sorry` のまま残すと、`propext`・`Classical.choice`・`Quot.sound` のほかに `sorryAx` に頼るので、E604 になります。

証明のファイルは、何かをビルドする前に文字として読みます。他人が書いた証明を CI でビルドすると、そこに書かれたコードがビルドの中で走るからです。`Kaname` の外の import、コードを走らせるコマンド（`#eval`、`run_cmd`）、構文を足すコマンド（`macro`、`syntax`、`elab`）、`native_decide`、検査を切るオプションや属性は書けません。

## Lean を入れる

Lean を走らせるのは `--lean` だけです。Lean のバージョン管理の道具 [elan](https://github.com/leanprover/elan) を入れてください。elan が最初に走らせる `lake` が、証明が使う toolchain の `leanprover/lean4:v4.34.0` を一度だけダウンロードします。ライブラリと合わせて約 2.7 GB です。`lake` が無ければ、`--lean` は理由を言って終了コード 2 で止まります。

kaname は、このサイトのブラウザで試すページでは、まだ開けません。開けるようになれば、性質は Lean を使わずにベクタで試され、W603 になります。証明を確かめる `--lean` は、手元の機械で走らせるものです。

[リファレンス](https://github.com/i2y/ritsu/blob/main/crates/kaname/docs/reference.md#properties)（英語）に、名前の決まり、まだ Lean に書けないもの、証明のファイルに書けないもの、終了コードがあります。証明は[スキル](https://github.com/i2y/ritsu/blob/main/skills/kaname/SKILL.md)を使ってエージェントに書かせることもできます。
