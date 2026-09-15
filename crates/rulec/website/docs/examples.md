# Examples

Every rule on this page is **one the repository's tests run on every commit**: it passes
`rulec check`, its own examples execute, and the reference evaluator and every generated
language are held to the same answers byte for byte. Copy any of them and it works.

They are ordered smallest first.

## A date decides which period it is

The smallest example there is: one input, one output. The rows are adjacent date ranges laid end to end with nothing between them.

```rule
rule 期間区分(period) v1
description "注文日で適用期間を決める。両端含みの隣接敷き詰めを行使する"
# 通算日で持たないと、月末と翌月初のあいだに実在しない整数の隙間ができて偽の E101 が出る（§2.1）

enum 期間(kind) = 改定前(before) | 春季(spring) | 通常(normal) | 年末(year_end)

inputs
  注文日(order_date) : date  range >=2026-01-01 <=2026-12-31

outputs
  区分(kind) : 期間

table 期間判定(pick)
policy unique
| 注文日                    | -> 区分(kind) : 期間 |
| <=2026-03-31              | 改定前               |
| >=2026-04-01 <=2026-06-30 | 春季                 |
| >=2026-07-01 <=2026-11-30 | 通常                 |
| >=2026-12-01              | 年末                 |

examples
| 注文日     | -> 区分 |
| 2026-03-31 | 改定前  |
| 2026-04-01 | 春季    |
| 2026-06-30 | 春季    |
| 2026-07-01 | 通常    |
| 2026-12-01 | 年末    |
```

**What this one shows**

- A date has **comparison and range, and nothing else** — no addition, no subtraction.
- The checker knows `<=2026-03-31` and `>=2026-04-01` are adjacent, because a date is held internally as a day number. Held as `20260331` instead, there would be a phantom gap between the end of one month and the start of the next, and the completeness check would report a hole that is not there.
- The output is not a number, so no `round` is required.

## A shipping fee with three conditions crossing

The design sketch written out as a rule. A boolean definition used as a column, a rate output, the `default` mark and `policy first` shadowing all appear at once.

```rule
rule 送料(shipping_fee) v4
description "設計文書 §1.2 のスケッチを規則にしたもの。真偽定義を列に使う正常系と、率の出力、default の印、policy first の要確認 1 対を行使する"

import std/都道府県

enum 会員区分(member_kind) = 一般(basic) default | ゴールド(gold) default | プラチナ(platinum)
group 遠隔地(remote) = 北海道, 沖縄県

inputs
  届け先(dest)    : 都道府県
  重量(weight)    : mass[g]        range >=1g <=40kg
  注文金額(total) : money[円, incl_tax]  range >=0円 <=1000万円
  会員(member)    : 会員区分

outputs
  送料(fee) : money[円, incl_tax]  round up(10円)

# 真偽の定義。条件は入力ひとつを見るだけなので、展開すれば区画の足し合わせになる（§5.3、§6.2）
define 大口(bulk) : bool = 注文金額 >= 3万円

table 基本送料(base_fee)
policy unique
| 届け先      | 重量    | -> 基本送料(base) : money[円, incl_tax] |
| 遠隔地      | <=2000g | 1200円                                  |
| 遠隔地      | >2000g  | 1800円                                  |
| not: 遠隔地 | <=2000g | 800円                                   |
| not: 遠隔地 | >2000g  | 1100円                                  |

table 負担判定(payer)
policy first
| 大口 | 会員     | -> 負担率(pay_rate) : rate |
| true | -        | 0%                         |
| -    | プラチナ | 50%                        |
| -    | -        | 100%                       |

result 送料 = 基本送料 × 負担率

examples
| 届け先 | 重量  | 注文金額 | 会員     | -> 送料 |
| 沖縄県 | 2500g | 40000円  | 一般     | 0円     |
| 東京都 | 1999g | 12000円  | プラチナ | 400円   |
| 北海道 | 500g  | 5000円   | 一般     | 1200円  |
```

**What this one shows**

- `define … : bool` is **a named boolean you can put in a column**. Naming the condition is what makes the table readable.
- `default` declares that a value needs no row of its own. Without it you get "appears in no row".
- Under `policy first` an earlier row hides a later one. The checker sorts those into three kinds and counts them, so the staircase does not drown the one pair that matters.

## Transcribing a real published tariff

Japan Post's base tariff, shipping from Tokyo. 47 prefectures × 7 sizes = 329 combinations, folded into 42 rows by six groups. **This is where the tool first pays for itself**: drop one prefecture and it stops before anything runs, naming that prefecture.

```rule
rule ゆうパック運賃(yupack_fee) v1
description "東京から差し出す場合の基本運賃"

import std/都道府県

enum サイズ区分(size_class) = S60(s60) | S80(s80) | S100(s100) | S120(s120) | S140(s140) | S160(s160) | S170(s170)

group 都内(tokyo)          = 東京都
group 北海道九州(hk)       = 北海道, 福岡県, 佐賀県, 長崎県, 熊本県, 大分県, 宮崎県, 鹿児島県
group 近距離圏(near)       = 青森県, 岩手県, 宮城県, 秋田県, 山形県, 福島県, 茨城県, 栃木県, 群馬県, 埼玉県, 千葉県, 神奈川県, 山梨県, 新潟県, 長野県, 富山県, 石川県, 福井県, 岐阜県, 静岡県, 愛知県, 三重県
group 近畿圏(kinki)        = 滋賀県, 京都府, 大阪府, 兵庫県, 奈良県, 和歌山県
group 中国四国(cs)         = 鳥取県, 島根県, 岡山県, 広島県, 山口県, 徳島県, 香川県, 愛媛県, 高知県
group 沖縄(okinawa)        = 沖縄県

inputs
  あて先(dest)     : 都道府県
  三辺合計(girth)  : length[cm]   range >=1cm <=170cm
  重量(weight)     : mass[g]    range >=1g <=25kg  contract_only

outputs
  運賃(fee) : money[円, incl_tax]  round up(10円)

table サイズ判定(size_of)  # 出典: 日本郵便 基本運賃表（東京）のサイズ区分
policy first
| 三辺合計 | -> サイズ(size) : サイズ区分 |
| <=60cm   | S60                          |
| <=80cm   | S80                          |
| <=100cm  | S100                         |
| <=120cm  | S120                         |
| <=140cm  | S140                         |
| <=160cm  | S160                         |
| -        | S170                         |

table 運賃表(fee_table)  # 出典: 日本郵便 基本運賃表（東京）
policy unique
| あて先     | サイズ | -> 運賃(fee) : money[円, incl_tax] |
| 都内       | S60    | 820円                              |
| 都内       | S80    | 1130円                             |
| 都内       | S100   | 1450円                             |
| 都内       | S120   | 1770円                             |
| 都内       | S140   | 2120円                             |
| 都内       | S160   | 2450円                             |
| 都内       | S170   | 3000円                             |
| 近距離圏   | S60    | 880円                              |
| 近距離圏   | S80    | 1200円                             |
| 近距離圏   | S100   | 1500円                             |
| 近距離圏   | S120   | 1830円                             |
| 近距離圏   | S140   | 2170円                             |
| 近距離圏   | S160   | 2500円                             |
| 近距離圏   | S170   | 3070円                             |
| 近畿圏     | S60    | 990円                              |
| 近畿圏     | S80    | 1310円                             |
| 近畿圏     | S100   | 1620円                             |
| 近畿圏     | S120   | 1940円                             |
| 近畿圏     | S140   | 2300円                             |
| 近畿圏     | S160   | 2610円                             |
| 近畿圏     | S170   | 3750円                             |
| 中国四国   | S60    | 1150円                             |
| 中国四国   | S80    | 1440円                             |
| 中国四国   | S100   | 1780円                             |
| 中国四国   | S120   | 2080円                             |
| 中国四国   | S140   | 2440円                             |
| 中国四国   | S160   | 2750円                             |
| 中国四国   | S170   | 3890円                             |
| 北海道九州 | S60    | 1410円                             |
| 北海道九州 | S80    | 1710円                             |
| 北海道九州 | S100   | 2020円                             |
| 北海道九州 | S120   | 2340円                             |
| 北海道九州 | S140   | 2680円                             |
| 北海道九州 | S160   | 3010円                             |
| 北海道九州 | S170   | 4140円                             |
| 沖縄       | S60    | 1450円                             |
| 沖縄       | S80    | 1810円                             |
| 沖縄       | S100   | 2160円                             |
| 沖縄       | S120   | 2490円                             |
| 沖縄       | S140   | 2860円                             |
| 沖縄       | S160   | 3180円                             |
| 沖縄       | S170   | 4350円                             |

examples
| あて先 | 三辺合計 | 重量 | -> 運賃 |
| 東京都 | 55cm     | 1kg  | 820円   |
| 沖縄県 | 100cm    | 3kg  | 2160円  |
| 北海道 | 61cm     | 20kg | 1710円  |
```

**What this one shows**

- A `group` names part of an enum. It is always expanded back to the values for checking, so **whether the grouping is an exact partition of the 47** is checked too.
- `import std/都道府県` brings the 47 values in, each with an ASCII alias.
- 42 rows, and `policy unique` still proves **reordering them cannot change the answer**.

## Two outputs at once

For one coupon: whether it applies, and how much it takes off. Stacking several coupons — the order and the loop — stays with the caller.

```rule
rule クーポン一枚(coupon_step) v1
description "クーポン1枚の適用可否と素割引（設計文書 §5.4 のスケッチ）。重ね掛けの順序と反復は呼び出し側が持つ。複数出力と、出力セルの名前を行使する"

enum クーポン種別(coupon_kind) = 率引き(percent) | 額引き(fixed) | 送料無料(free_ship)

inputs
  商品合計(subtotal)  : money[円, incl_tax]  range >=0円 <=100万円
  適用済割引(applied) : money[円, incl_tax]  range >=0円 <=100万円
  種別(kind)          : クーポン種別
  割引率(rate)        : rate[step 1%]     range >=0% <=100%
  額面(face)          : money[円, incl_tax]  range >=0円 <=10万円
  同商品適用済(dup)   : bool

outputs
  可否(ok)    : bool
  素割引(raw) : money[円, incl_tax]  round down(1円)

# 端数処理は公開規約に記載がないので切り捨てに仮置き（§7.1）
derive 残余(margin) : money[円, incl_tax] = 商品合計 - 適用済割引 - 額面  range >=-110万円 <=100万円

define 率割引(rate_off) : money[円, incl_tax] = 商品合計 × 割引率

table 適用可否(applicable)
policy first
| 種別   | 残余  | 同商品適用済 | -> 可否(ok) : bool |
| -      | -     | true         | false              |
| 額引き | <=0円 | -            | false              |
| -      | -     | -            | true               |

table 素の割引(raw_discount)
policy unique
| 可否  | 種別     | -> 素割引(raw) : money[円, incl_tax] |
| false | -        | 0円                                  |
| true  | 率引き   | 率割引                               |
| true  | 額引き   | 額面                                 |
| true  | 送料無料 | 0円                                  |

examples
| 商品合計 | 適用済割引 | 種別     | 割引率 | 額面  | 同商品適用済 | -> 可否 | 素割引 |
| 10000円  | 0円        | 率引き   | 10%    | 0円   | false        | true    | 1000円 |
| 10000円  | 0円        | 額引き   | 0%     | 500円 | false        | true    | 500円  |
| 400円    | 0円        | 額引き   | 0%     | 500円 | false        | false   | 0円    |
| 10000円  | 0円        | 率引き   | 10%    | 0円   | true         | false   | 0円    |
| 10000円  | 0円        | 送料無料 | 0%     | 0円   | false        | true    | 0円    |
```

**What this one shows**

- **There can be two or more outputs.** Two tables fill one each, and each carries its own `round`.
- An output cell holds **one value or one name**. The arithmetic moves to a `define`, so the table keeps the branching and nothing else.
- The first table's output (`可否`) is a column of the second. Items run one way, top to bottom, so the dependencies are always readable off the page.

## Eligibility and amount together

The same subject in another shape: a `derive` computes what is left, and that becomes a column.

```rule
rule クーポン割引(coupon_discount) v1
description "クーポン1枚の適用可否と割引額。順序と反復は呼び出し側。出典: 楽天/Yahoo の公開ヘルプから再構成"
# ※ 出力セルに名前を書ける（§3.2 で採用済み）。式は書けないので、率引きは定義を一枚挟む

enum 種別(kind)  = 率引き(rate) | 額引き(amount) | 送料無料(free_ship)
enum 適用範囲(scope) = 店内全商品(shop) default | 対象商品(item)

inputs
  元価(list)        : money[円,incl_tax]  range >=0円 <=1000万円   # 率引きの基準
  現在残額(running) : money[円,incl_tax]  range >=0円 <=1000万円   # 額引きの基準（率引き適用後）
  クーポン種別(kind) : 種別
  クーポン範囲(scope): 適用範囲
  割引率(rate)      : rate[step 1%]
  割引額面(face)    : money[円,incl_tax]  range >=0円 <=100万円
  上限額(cap)       : money[円,incl_tax]  range >=0円 <=100万円
  最低購入額(min)   : money[円,incl_tax]  range >=0円 <=100万円
  同商品適用済(used): bool
  期限内(alive)     : bool

outputs
  割引額(discount) : money[円,incl_tax]  round down(1円)

# [導出] 実表が要求した形。どちらも「入力 − 入力」の線形結合
derive 最低差(min_gap) : money[円,incl_tax] = 現在残額 − 最低購入額  range >=-100万円 <=1000万円
derive 支払差(pay_gap) : money[円,incl_tax] = 現在残額 − 割引額面    range >=-100万円 <=1000万円

table 適用可否(applicable)
policy first
| 期限内 | 同商品適用済 | クーポン範囲 | クーポン種別 | 最低差 | 支払差 | -> 可否(ok) : bool |
| false  | -            | -            | -            | -      | -      | false              |
| -      | true         | 対象商品     | -            | -      | -      | false              |
| -      | -            | -            | -            | <0円   | -      | false              |
| -      | -            | -            | 額引き       | -      | <=0円  | false              |
| -      | -            | -            | -            | -      | -      | true               |

# 率引きは元価基準、額引きは割引後（現在残額）基準 — 楽天の原文どおり
define 率割引(rate_off) : money[円,incl_tax] = 元価 × 割引率

table 素の割引(raw_discount)
policy unique
| 可否  | クーポン種別 | -> 素割引(raw) : money[円,incl_tax] |
| false | -            | 0円                                 |
| true  | 率引き       | 率割引                              |
| true  | 額引き       | 割引額面                            |
| true  | 送料無料     | 0円                                 |

result 割引額 = down(min(素割引, 上限額), 1円)

examples
| 元価    | 現在残額 | クーポン種別 | クーポン範囲 | 割引率 | 割引額面 | 上限額 | 最低購入額 | 同商品適用済 | 期限内 | -> 割引額 |
| 10000円 | 10000円  | 率引き       | 店内全商品   | 10%    | 0円      | 1000円 | 5000円     | false        | true   | 1000円    |
| 10000円 | 9000円   | 額引き       | 店内全商品   | 0%     | 500円    | 500円  | 5000円     | false        | true   | 500円     |
| 10000円 | 400円    | 額引き       | 店内全商品   | 0%     | 500円    | 500円  | 0円        | false        | true   | 0円       |
```

**What this one shows**

- A `derive` is a named amount built from **additions and subtractions of inputs only**. It is the one intermediate value that can sit in a column as a quantity, and it must declare a `range`.
- If the declared range does not contain the values that can actually occur, E112 stops it — the range is the universe the checks reason over.

## When the checker could not decide

Two derived values share an input, so whether two rows can fire together is not decidable here. The tool neither waves it through nor invents an error: **it warns, and puts a runtime guard in the generated code.**

```rule
rule クーポン併用(coupon_stack) v1
description "二つの導出が入力を共有する 一意 の表。W114 と生成コードのガードの題材（§6.2、§11）"

enum 判定(verdict) = 対象外(no) default | 対象(yes)

inputs
  合計(total)   : money[円,incl_tax]  range >=0円 <=100万円
  割引A(disc_a) : money[円,incl_tax]  range >=0円 <=10万円
  割引B(disc_b) : money[円,incl_tax]  range >=0円 <=10万円

outputs
  併用可否(verdict) : 判定

# 割引B は 0 円以上なので、残高B は定義から必ず 残高A 以下になる。
# つまり 残高A <= 1000円 のとき 残高B が 3980円 に届くことはない。
# 導出ごとに独立な区間の篩はこの従属を見ないので、下の 行1 と 行2 の重なりは
# 実現不能と証明できないまま残る（W114）。Fourier-Motzkin なら証明できる形。
derive 残高A(rest_a) : money[円,incl_tax] = 合計 - 割引A            range >=-10万円 <=100万円
derive 残高B(rest_b) : money[円,incl_tax] = 合計 - 割引A - 割引B    range >=-20万円 <=100万円

table 適用判定(decide)
policy unique
| 残高A    | 残高B    | -> 併用可否(verdict) : 判定 |
| <=1000円 | -        | 対象外                      |
| -        | >=3980円 | 対象                        |
| >1000円  | <3980円  | 対象外                      |

examples
| 合計    | 割引A  | 割引B  | -> 併用可否 |
| 500円   | 0円    | 0円    | 対象外      |
| 10000円 | 1000円 | 1000円 | 対象        |
| 5000円  | 2000円 | 2000円 | 対象外      |
```

**What this one shows**

- W114 says "not proved". It does not say the overlap exists, and it does not say it doesn't.
- The generated code refuses to silently pick the earlier row if such an input ever arrives; it raises instead. **If that guard ever fires, the overlap was real.**
- This is the only place where something undecided statically is carried into runtime.

## The answer is an order

Which of two coupons applies first. The sorting itself is the caller's loop, but **the comparison lives in the rule**, because that is the part someone has to approve.

```rule
rule 適用順序(apply_order) v1
description "クーポン二枚のどちらを先に適用するか（§5.4）。反復と並べ替えは呼び出し側で、比較だけが規則になる"
# 楽天の規約は「率引きを先に、次に額引き、額引きどうしは有効期限が近い順」

enum 種別(kind)  = 率引き(rate) | 額引き(amount)
enum 先後(order) = 先(first) | 後(second)

inputs
  A種別(a_kind) : 種別
  B種別(b_kind) : 種別
  A期限(a_due)  : date
  B期限(b_due)  : date

outputs
  A順序(a_order) : 先後

# 日付どうしの比較。導出にできない型なので、名前どうしを直接比べる第二の原子（§5.3）。
define Aが早いか同じ(a_earlier) : bool = A期限 <= B期限

table 順序判定(decide)
policy first
| A種別  | B種別  | Aが早いか同じ | -> A順序(a_order) : 先後 |
| 率引き | 額引き | -             | 先                       |
| 額引き | 率引き | -             | 後                       |
| -      | -      | true          | 先                       |
| -      | -      | -             | 後                       |

examples
| A種別  | B種別  | A期限      | B期限      | -> A順序 |
| 率引き | 額引き | 2026-12-31 | 2026-01-01 | 先       |
| 額引き | 率引き | 2026-01-01 | 2026-12-31 | 後       |
| 額引き | 額引き | 2026-04-01 | 2026-05-01 | 先       |
| 額引き | 額引き | 2026-05-01 | 2026-04-01 | 後       |
| 率引き | 率引き | 2026-04-01 | 2026-04-01 | 先       |
```

**What this one shows**

- Of the four kinds of answer — amount, yes/no, class, order — this is the fourth.
- The inputs are the attributes of both coupons and the output is "first" or "second", which keeps it to one decision.
- Two dates get compared here.

## Apportioning, one line at a time

Spreading one discount across the lines of an order. **One line is one decision**; the loop and the running remainder stay with the caller, who walks the lines subtracting as it goes. **The total comes out exact by construction** (checked over 2,000 generated sets of lines).

```rule
rule 値引の充当(allocate) v1
description "一括値引きを明細に配分する。1 明細 = 1 回の判定で、反復と順序は呼び出し側が持つ"

inputs
  明細定価(list)      : money[円, incl_tax]  range >=0円 <=100万円
  残り値引(remaining) : money[円, incl_tax]  range >=0円 <=100万円
  対象(eligible)      : bool

outputs
  充当額(applied) : money[円, incl_tax]  round down(1円)

define 充てられる額(cap) : money[円, incl_tax] = min(明細定価, 残り値引)

table 充当可否(applies)
policy unique
| 対象  | -> 充当(on) : money[円, incl_tax] |
| true  | 充てられる額                      |
| false | 0円                               |

result 充当額 = 充当

examples
| 明細定価 | 残り値引 | 対象  | -> 充当額 |
| 3000円   | 1000円   | true  | 1000円    |
| 500円    | 1000円   | true  | 500円     |
| 3000円   | 1000円   | false | 0円       |
```

**What this one shows**

- The rule decides how to allocate; the caller does the walking.
- `min` is available, so "up to this line's own value" fits on one line.
- Splitting by ratio (`line ÷ total`) **cannot be written**: division is only allowed by a constant.

## A rate finer than one percent

A per-contract fee rate turned into the fee on one transaction. The rate moves in tenths of a percent, so `0.5%` is a value you can write.

```rule
rule 決済手数料(payment_fee) v1
description "契約ごとに決まる手数料率から取引一件の手数料を出す。率が 1% より細かい刻みで決まる例"

inputs
  取引額(amount) : money[円, incl_tax]  range >=0円 <=100万円
  手数料率(rate) : rate[step 0.1%]      range >=0% <=10%
  少額(small)    : bool

outputs
  手数料(fee) : money[円, incl_tax]  round up(1円)

# 0.5% 以下は徴収しない、という契約上の下限（率の刻みが 0.1% なのでこの行が書ける）
define 率手数料(rate_fee) : money[円, incl_tax] = 取引額 × 手数料率

table 手数料表(fee_table)
policy unique
| 手数料率 | 少額  | -> 手数料(fee) : money[円, incl_tax] |
| <=0.5%   | -     | 0円                                  |
| >0.5%    | true  | 50円                                 |
| >0.5%    | false | 率手数料                             |

result 手数料 = 手数料

examples
| 取引額  | 手数料率 | 少額  | -> 手数料 |
| 10000円 | 0.5%     | false | 0円       |
| 10000円 | 0.6%     | false | 60円      |
| 10000円 | 3.6%     | false | 360円     |
| 10000円 | 3.6%     | true  | 50円      |
```

**What this one shows**

- `rate[step 0.1%]` declares that at runtime the value is a whole number of tenths of a percent: 0.5% travels as 5, 10% as 100.
- **A value between two steps cannot be written.** `0.05%` in that column stops at E114 — quietly moving it to the nearest step would put a different boundary in the code than the one on the page.
- The arithmetic stays a rate until the end, where it is rounded exactly once, because where to round is a business decision.

## Scores added up, then ranked by how much of the total they reach

No money anywhere in this one. Four scored criteria are added with weights, and the rank comes from how much of the maximum the total reaches. This is the shape of rule that is *not* written as a table today but becomes one the moment somebody writes it down.

```rule
rule 評価ランク(rank) v1
description "四つの評価項目を重み付きで合算し、満点に対する達成率でランクを決める。金額の出てこない例"

enum ランク(grade) = S(s) | A(a) | B(b) | C(c)

inputs
  品質(quality)  : number  range >=0 <=10
  納期(delivery) : number  range >=0 <=10
  価格(price)    : number  range >=0 <=10
  対応(support)  : number  range >=0 <=10

outputs
  評価(grade) : ランク

# 重みは 2:1:1:1、満点は 50 点
derive 合計点(total) : number = 品質 × 2 + 納期 + 価格 + 対応  range >=0 <=50
define 達成率(ratio) : rate = 合計点 ÷ 50

table ランク表(grade_table)
policy first
| 達成率 | -> 評価(grade) : ランク |
| >=90%  | S                       |
| >=80%  | A                       |
| >=60%  | B                       |
| -      | C                       |

result 評価 = 評価

examples
| 品質 | 納期 | 価格 | 対応 | -> 評価 |
| 10   | 10   | 10   | 10   | S       |
| 9    | 8    | 8    | 7    | A       |
| 6    | 6    | 6    | 6    | B       |
| 3    | 3    | 3    | 3    | C       |
```

**What this one shows**

- A `derive` is a name for **additions, subtractions and integer multiples of the inputs**, which is exactly what a weighted total is.
- Declaring `define 達成率 : rate = 合計点 ÷ 50` lets the cells say `>=90%` — **the threshold stays a proportion on the page**. Whether a dimensionless value is called a rate or a number is the declaration's to say.
- **No division happens at runtime.** `>=90%` compiles to `合計点 >= 45`, because a constant maximum folds the boundary into a constant. A maximum that is an *input* cannot be written: that is division by a variable, and E115 stops it — take the proportion itself as a `rate` input instead.

## Returning a number with no unit

One point per 100 yen. Dividing money by money cancels the unit and leaves a `number` — a whole number carrying none.

```rule
rule ポイント付与(points) v1
description "会計 1 件で貯まる点数。100 円につき 1 点で、区分とキャンペーンで倍になる。単位のない数を使う例"

enum 会員区分(member_kind) = 一般(basic) | 上級(gold)

inputs
  税込金額(paid)         : money[円, incl_tax]  range >=0円 <=100万円
  区分(kind)             : 会員区分
  キャンペーン(campaign) : bool

outputs
  点数(pts) : number  round down(1)

# 100 円につき 1 点。端数は最後に一度だけ落とす（§7.2）
define 基本点(base)  : number = 税込金額 ÷ 100円
define 倍付け(boost) : number = 基本点 × 2

table 点数表(pts_table)
policy unique
| 区分 | キャンペーン | -> 点数(pts) : number |
| 上級 | -            | 倍付け                |
| 一般 | true         | 倍付け                |
| 一般 | false        | 基本点                |

result 点数 = 点数

examples
| 税込金額 | 区分 | キャンペーン | -> 点数 |
| 1050円   | 一般 | false        | 10      |
| 1050円   | 上級 | false        | 21      |
| 1050円   | 一般 | true         | 21      |
| 99円     | 一般 | false        | 0       |
```

**What this one shows**

- `number` is the type for **counts, days and scores**: whole numbers with no unit.
- `税込金額 ÷ 100円` **does not divide the integer**. It multiplies the internal step by 100, so 1050 yen stays 10.5 points all the way to the end, where `round down(1)` makes it 10 exactly once. Python's `//` and Go's `/` truncate in different directions; neither gets a say here.
- Division is allowed **by a constant only**. If the divisor is business data, it belongs in the table as a rate or a constant column.

## Three tables stacked, two outputs returned

Weight gives a weight class, the class and the membership give a tier, and the tier with the amount payable gives shipping and a multiplier. What one table produces is a column of the next. Two things come back: the amount charged and the points.

```rule
rule 会員特典(member_perk) v1
description "会員区分と重量から請求額とポイントを一度に出す。出力が二つ、result が一本、率が表の列から来る例"

enum 会員(member) = 一般(basic) | 上位(gold)
enum 重さ区分(wclass) = 軽(light) | 中(mid) | 重(heavy)
enum 帯(tier) = 銅(bronze) | 銀(silver) | 金(gold)

inputs
  会員区分(m)     : 会員
  重量(weight)    : mass[g]              range >=1g <=20kg
  商品合計(gross) : money[円, incl_tax]   range >=0円 <=100万円
  値引(off)       : money[円, incl_tax]   range >=0円 <=10万円

# 宣言順（請求額 → 付与点）と文字の並び順（付与点 → 請求額）がわざと逆。
# 生成コードの返す順序が言語ごとにぶれていないかは、この順序でしか出ない。
outputs
  請求額(total) : money[円, incl_tax]  round half_up(10円)
  付与点(pts)   : number               round down(1)

derive 支払額(net) : money[円, incl_tax] = 商品合計 - 値引  range >=-10万円 <=100万円

# 金額を金額の定数で割ると単位が消えて number になる（§2.3）
define 基本点(base) : number = 商品合計 ÷ 100円

table 重さ判定(w_of)
policy first
| 重量   | -> 区分(wc) : 重さ区分 |
| <=2kg  | 軽                     |
| <=10kg | 中                     |
| -      | 重                     |

table 帯判定(tier_of)
policy unique
| 区分 | 会員区分 | -> 帯(t) : 帯 |
| 軽   | 一般     | 銅            |
| 軽   | 上位     | 銀            |
| 中   | 一般     | 銀            |
| 中   | 上位     | 金            |
| 重   | -        | 金            |

# 一つの表が二つの列を出し、そのうち率の列を下の define が使う。
# 率の刻み（10%）が列の保持スケールに入らないと、付与点が 10 倍になる。
table 送料表(ship)
policy unique
| 帯 | 支払額   | -> 送料(s) : money[円, incl_tax] | -> 倍率(r) : rate[step 10%] |
| 銅 | <3000円  | 800円                            | 100%                        |
| 銅 | >=3000円 | 400円                            | 100%                        |
| 銀 | -        | 300円                            | 120%                        |
| 金 | -        | 0円                              | 150%                        |

# 二つ目の出力は、その出力と同じ名前の define から取る（result は最初の出力にしか効かない）
define 付与点(pts) : number = 基本点 × 倍率

result 請求額 = max(支払額, 0円) + 送料

examples
| 会員区分 | 重量  | 商品合計 | 値引   | -> 請求額 | -> 付与点 |
| 一般     | 1kg   | 2000円   | 500円  | 2300円    | 20        |
| 上位     | 5kg   | 20000円  | 2000円 | 18000円   | 300       |
| 一般     | 15kg  | 5000円   | 0円    | 5000円    | 75        |
```

**What this one shows**

- **A table's output column is a column of any later table.** There is no limit on the depth (past the check's budget it stops at E109). When `rulec check` fails it names the row that fired in each of them: `table 重さ判定 row 2 / table 帯判定 row 4 / table 送料表 row 4`.
- **One table may produce several output columns.** `送料表` produces `送料` and `倍率` at once, and the `define` below it uses that rate. A rate keeps its step (`step 10%`) through the column, so `基本点 × 倍率` is rounded exactly once, at the end.
- **A `derive` can be a column.** Declaring `支払額 = 商品合計 - 値引` turns judging on the amount after the discount into one column of `送料表` rather than one bare line of arithmetic.
- **`result` assembles the first output and nothing else** (E015). The second and later ones are taken from a `define` of the same name - `define 付与点` here. A second `result` line stops at E016.

## A rule written in English — EU air passenger rights

Names and cells are English, so not one ASCII alias appears. The money is EUR and the distance is km. It is a transcription of published law — Article 7 of Regulation (EC) No 261/2004 — whose text is already shaped like a decision table.

```rule
rule ec261 v1
description "EU 旅客権利規則 (EC) No 261/2004 第7条。英語圏の規約と EUR・km を行使する"

# 公開されている法令の第7条をそのまま写したもの。金額は 1 項、5 割引きの条件は 2 項。
# 距離と「域内かどうか」の二つで帯が決まり、同じ帯が金額と時間の閾値の両方を決める。

enum band = short | medium | long

inputs
  distance : length[km]  range >=1km <=20000km
  intra_eu : bool
  delay    : number      range >=0 <=48

outputs
  compensation : money[EUR, incl_tax]  round down(1EUR)

# 7 条 1 項。(a) 1500km 以下、(b) 域内の 1500km 超と、域外の 1500〜3500km、(c) それ以外。
# 「between 1500 and 3500」が両端を含むかは条文から読めない。(a) が「1500km 以下」なので
# 下端は開き、ここでそう決める — 決めなければ表が書けない、というのがこの道具の要点である。
table band_of
policy unique
| distance          | intra_eu | -> band : band |
| <=1500km          | -        | short          |
| >1500km           | true     | medium         |
| >1500km <=3500km  | false    | medium         |
| >3500km           | false    | long           |

table amount
policy unique
| band   | -> base : money[EUR, incl_tax] |
| short  | 250EUR                         |
| medium | 400EUR                         |
| long   | 600EUR                         |

# 7 条 2 項。代替便の到着が (a) 2 時間 (b) 3 時間 (c) 4 時間 を超えなければ 5 割にできる。
# 条文の (a)(b)(c) は 1 項の距離の条件をそのまま書き直しているので、ここでも帯ではなく
# 距離で引く。帯で引くと二行の表で済むが、そのぶん「どの距離なら 4 時間か」が条文から
# 一段遠くなる。
table reduction
policy unique
| distance         | intra_eu | delay | -> factor : rate[step 50%] |
| <=1500km         | -        | <=2   | 50%                        |
| <=1500km         | -        | >2    | 100%                       |
| >1500km          | true     | <=3   | 50%                        |
| >1500km          | true     | >3    | 100%                       |
| >1500km <=3500km | false    | <=3   | 50%                        |
| >1500km <=3500km | false    | >3    | 100%                       |
| >3500km          | false    | <=4   | 50%                        |
| >3500km          | false    | >4    | 100%                       |

result compensation = base × factor

examples
| distance | intra_eu | delay | -> compensation |
| 900km    | true     | 5     | 250EUR          |
| 900km    | true     | 2     | 125EUR          |
| 2000km   | true     | 5     | 400EUR          |
| 3000km   | false    | 3     | 200EUR          |
| 6000km   | false    | 5     | 600EUR          |
| 6000km   | false    | 4     | 300EUR          |
```

**What this one shows**

- **An ASCII name needs no alias.** A kanji cannot begin an exported Go identifier, which is why `運賃(fee)` carries one; `distance` does not. Write the rule in English and there are no parentheses anywhere.
- **Two currencies never convert.** `100円` in a `money[EUR]` column stops at E103. There is no exchange rate in this tool and there must not be one ([the units](reference.md)).
- **What the text leaves open, the table makes you decide.** Article 7(1)(b) says "between 1500 and 3500 kilometres" and does not say whether either end is included. Since (a) is "1500 kilometres or less", the lower end is open here — and that is a decision, made in the open. Leave it undecided and the checker stops with a gap or an overlap.
- **Sometimes writing the same condition twice is the faithful thing.** The 50% reduction thresholds could be keyed on the band in two columns, but Article 7(2) restates the distance conditions in full. Keying them on distance keeps the rows one-for-one with the text.

---

[Write a table (.rule)](tour.md){ .md-button .md-button--primary }
[Grammar](reference.md){ .md-button }
