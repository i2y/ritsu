# Examples

Every rule on this page is **one the repository's tests run on every commit**: it passes
`rulec check`, its own examples execute, and the reference evaluator and every generated
language are held to the same answers byte for byte. Copy any of them and it works.

They are ordered smallest first.

## A date decides which period it is

The smallest example there is: one input, one output. The rows are adjacent date ranges laid end to end with nothing between them.

```rule
rule 期間区分(period) v1
description "注文日で適用期間を決める。両端を含む日付の範囲を、隙間なく隣り合わせに並べた例"
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
description "設計文書 §1.2 のスケッチを規則にしたもの。真偽定義を列に使う正常系と、率の出力、default の印、policy first の要確認 1 対が出てくる"

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
description "クーポン1枚の適用可否と素割引（設計文書 §5.4 のスケッチ）。重ね掛けの順序と反復は呼び出し側が持つ。複数出力と、出力セルの名前が出てくる"

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
# 検査は導出を一本ずつ独立に見るので、この結びつきが見えない。だから下の 行1 と 行2 の
# 重なりは、起こりえないと証明できないまま残る（W114）。Fourier-Motzkin なら証明できる形。
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
| 帯 | 支払額   | -> 送料(s) : money[円, incl_tax] | 倍率(r) : rate[step 10%] |
| 銅 | <3000円  | 800円                            | 100%                     |
| 銅 | >=3000円 | 400円                            | 100%                     |
| 銀 | -        | 300円                            | 120%                     |
| 金 | -        | 0円                              | 150%                     |

# 二つ目の出力は、その出力と同じ名前の define から取る（result は最初の出力にしか効かない）
define 付与点(pts) : number = 基本点 × 倍率

result 請求額 = max(支払額, 0円) + 送料

examples
| 会員区分 | 重量 | 商品合計 | 値引   | -> 請求額 | 付与点 |
| 一般     | 1kg  | 2000円   | 500円  | 2300円    | 20     |
| 上位     | 5kg  | 20000円  | 2000円 | 18000円   | 300    |
| 一般     | 15kg | 5000円   | 0円    | 5000円    | 75     |
```

**What this one shows**

- **A table's output column is a column of any later table.** There is no limit on the depth (past the check's budget it stops at E109). When `rulec check` fails it names the row that fired in each of them: `table 重さ判定 row 2 / table 帯判定 row 4 / table 送料表 row 4`.
- **One table may produce several output columns.** `送料表` produces `送料` and `倍率` at once, and the `define` below it uses that rate. A rate keeps its step (`step 10%`) through the column, so `基本点 × 倍率` is rounded exactly once, at the end.
- **A `derive` can be a column.** Declaring `支払額 = 商品合計 - 値引` turns judging on the amount after the discount into one column of `送料表` rather than one bare line of arithmetic.
- **`result` assembles the first output and nothing else** (E015). The second and later ones are taken from a `define` of the same name - `define 付与点` here. A second `result` line stops at E016.

## A sequence walked into one answer

The caller passes the rows of a tariff sheet and the rule walks them in order. A table judges one element at a time, and `fold` says what each verdict does: go on, halt, take this one, hold the best so far. It is **the one shape that takes a number of things that is not fixed**.

```rule
rule 全国運賃(freight) v1
description "呼び出し側が渡す運賃行を順に見て、一つの運賃に畳む。並びをたどる規則の例"

enum 採用区分(verdict) = スキップ(skip) | 打ち切り(halt) | 確定(take) | 持ち越し(hold)
enum ゾーン区分(zone) = 近畿圏(kinki) | 遠隔地(remote)

# 一件ぶんの欄。呼び出し側はこの欄のそろった要素を何件でも渡す
elements 運賃行(freight_rows)
  行ゾーン(row_zone) : ゾーン区分
  閾値(threshold)    : money[円, incl_tax]  range >=0円 <=100万円
  行運賃(row_fee)    : money[円, incl_tax]  range >=0円 <=10万円

outputs
  運賃(fee) : money[円, incl_tax]  round up(1円)

# 一件の要素に対する判定。表の検査はいままでどおり効く（この四行で 採用区分 を覆いきる）
table 行判定(row_of)
policy unique
| 行ゾーン | 閾値     | -> 採用(verdict) : 採用区分 |
| 近畿圏   | <=1000円 | 確定                        |
| 近畿圏   | >1000円  | 持ち越し                    |
| 遠隔地   | <=1000円 | スキップ                    |
| 遠隔地   | >1000円  | 打ち切り                    |

# 判定ごとの行き先。どの判定にも行き先が要る（E024）
fold 採用 over 運賃行
  スキップ  -> next
  打ち切り  -> stop with 0円
  確定      -> take_unique 行運賃
  持ち越し  -> keep_max 行運賃 by 閾値
  empty     -> 0円
  exhausted -> held

sequence 近い一件(near)
| 行ゾーン | 閾値   | 行運賃 |
| 近畿圏   | 500円  | 800円  |
| 近畿圏   | 2000円 | 1500円 |

sequence 空(none)
| 行ゾーン | 閾値 | 行運賃 |

examples
| 運賃行   | -> 運賃 |
| 近い一件 | 800円   |
| 空       | 0円     |
```

**What this one shows**

- **`elements` declares what one element carries.** The fields are declared exactly like `inputs`, ranges and units included, and the caller passes any number of elements with those fields filled in.
- **The table's own checks are unchanged.** One element is one case, so completeness, overlap and units are proved over it as they always were: the four rows above cover `採用区分` exactly.
- **The fold gives every verdict somewhere to go** — `next`, `stop with <value>`, `take_unique <value>` (a second element that also takes is a run-time error), `keep_max <value> by <key>`. A verdict with no arm stops at E024.
- **The answer for no elements, and for a walk that reached the end, are both required** (E022, E023). An empty sequence always turns up, and answering with what is held is a choice made by writing it (`exhausted -> held`).
- **An example names a `sequence`.** A cell holds one value, so the list is written under a name and the example points at it; a `sequence` with no rows is the example for a sequence with nothing in it.
- **SQL and NumPy are the two targets that do not get it.** One query has no place to carry a value from row to row and stop partway, and a walk that carries state from element to element is not a column operation. Every other target is generated, and agrees with the reference evaluator on every commit.

## Counting a sequence, and deciding from the count

An invoice's name is matched against the supplier ledger, one candidate at a time. A table judges each candidate, `count` counts the ones it called a match, and **the next table decides what to do with that number**. "One means automatic, more than one means look at it" is decided inside the rule rather than by whoever counted before calling it.

```rule
rule 納入先照合(supplier_match) v1
description "請求書の宛名を取引先台帳と一件ずつ照合し、一致した件数で次の手を決める。並びを数える例"

enum 照合(match_kind) = 一致(same) | 不一致(diff)
enum 次の手(action_kind) = 新規登録(register) | 自動確定(auto) | 目視確認(review)

inputs
  自動確定可(auto_ok) : bool

# 台帳の候補。一件ぶんの欄で、呼び出し側は何件でも渡す
elements 候補(candidates)
  会社名一致(name_match) : bool
  住所一致(addr_match)   : bool

outputs
  手続き(action) : 次の手

# 一件の候補に対する判定。表の検査はいままでどおり効く
table 候補判定(row_of)
policy unique
| 会社名一致 | 住所一致 | -> 照合結果(kind) : 照合 |
| true       | true     | 一致                     |
| true       | false    | 不一致                   |
| false      | -        | 不一致                   |

# 歩いたあとに残るのは数だけ。範囲は完全性の全体集合であり、並びの長さの上限でもある
count 一致数(hits) over 候補 where 照合結果 = 一致  range >=0 <=50

table 手続き判定(action_of)
policy unique
| 一致数 | 自動確定可 | -> 手続き(action) : 次の手 |
| 0      | -          | 新規登録                   |
| 1      | true       | 自動確定                   |
| 1      | false      | 目視確認                   |
| >=2    | -          | 目視確認                   |

sequence 一件だけ一致(one)
| 会社名一致 | 住所一致 |
| true       | true     |
| true       | false    |

sequence 二件一致(two)
| 会社名一致 | 住所一致 |
| true       | true     |
| true       | true     |

sequence 候補なし(none)
| 会社名一致 | 住所一致 |

examples
| 自動確定可 | 候補         | -> 手続き |
| true       | 一件だけ一致 | 自動確定  |
| false      | 一件だけ一致 | 目視確認  |
| true       | 二件一致     | 目視確認  |
| true       | 候補なし     | 新規登録  |
```

**What this one shows**

- **A count is what the walk leaves behind.** `count 一致数(hits) over 候補 where 照合結果 = 一致` is how many elements the per-element table judged `一致`. From there it is a `number`, so it can be a column.
- **What turns the number into a decision is an ordinary table.** A gap or an overlap in the `0` / `1` / `>=2` boundaries stops the check as it always would. Counting and deciding are checked separately.
- **The range says two things** (`range >=0 <=50`): the universe the completeness check quantifies over, and **the cap on the sequence**. Pass 51 candidates and the generated code refuses at the door — the same answer a number outside its range gets.
- **Nothing accumulates across elements.** A count counts; there is no sum and no average. Compute one before the call and pass it in as a value.
- **A `fold` and a `count` cannot share a rule** (E031): two endings for the same walk, and a fold may stop partway.

## A rule written in English — EU air passenger rights

Names and cells are English, so not one ASCII alias appears. The money is EUR and the distance is km. It is a transcription of published law — Article 7 of Regulation (EC) No 261/2004 — whose text is already shaped like a decision table.

```rule
rule ec261 v1
description "EU 旅客権利規則 (EC) No 261/2004 第7条。英語圏の規約と EUR・km の例"

# 公開されている法令の第7条をそのまま写したもの。金額は 1 項、5 割引きの条件は 2 項。
# 距離と「域内かどうか」の二つで帯が決まり、同じ帯が金額と時間の閾値の両方を決める。

enum band = short | medium | long

inputs
  distance : length[km]  range >=1km <=20000km
  intra_eu : bool
  delay    : duration[h]  range >=0h <=48h

outputs
  compensation : money[EUR, incl_tax]  round down(1EUR)

# 7 条 1 項。(a) 1500km 以下、(b) 域内の 1500km 超と、域外の 1500〜3500km、(c) それ以外。
# 「between 1500 and 3500」が両端を含むかは条文から読めない。(a) が「1500km 以下」なので
# 下端は開き、ここでそう決める — 決めなければ表が書けない、というのがこの道具の要点である。
table band_of
policy unique
| distance         | intra_eu | -> band : band |
| <=1500km         | -        | short          |
| >1500km          | true     | medium         |
| >1500km <=3500km | false    | medium         |
| >3500km          | false    | long           |

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
| <=1500km         | -        | <=2h  | 50%                        |
| <=1500km         | -        | >2h   | 100%                       |
| >1500km          | true     | <=3h  | 50%                        |
| >1500km          | true     | >3h   | 100%                       |
| >1500km <=3500km | false    | <=3h  | 50%                        |
| >1500km <=3500km | false    | >3h   | 100%                       |
| >3500km          | false    | <=4h  | 50%                        |
| >3500km          | false    | >4h   | 100%                       |

result compensation = base × factor

examples
| distance | intra_eu | delay | -> compensation |
| 900km    | true     | 5h    | 250EUR          |
| 900km    | true     | 2h    | 125EUR          |
| 2000km   | true     | 5h    | 400EUR          |
| 3000km   | false    | 3h    | 200EUR          |
| 6000km   | false    | 5h    | 600EUR          |
| 6000km   | false    | 4h    | 300EUR          |
```

**What this one shows**

- **An ASCII name needs no alias.** A kanji cannot begin an exported Go identifier, which is why `運賃(fee)` carries one; `distance` does not. Write the rule in English and there are no parentheses anywhere.
- **Two currencies never convert.** `100円` in a `money[EUR]` column stops at E103. There is no exchange rate in this tool and there must not be one ([the units](reference.md)).
- **What the text leaves open, the table makes you decide.** Article 7(1)(b) says "between 1500 and 3500 kilometres" and does not say whether either end is included. Since (a) is "1500 kilometres or less", the lower end is open here — and that is a decision, made in the open. Leave it undecided and the checker stops with a gap or an overlap.
- **Sometimes writing the same condition twice is the faithful thing.** The 50% reduction thresholds could be keyed on the band in two columns, but Article 7(2) restates the distance conditions in full. Keying them on distance keeps the rows one-for-one with the text.

## Stamp duty on a receipt

The table for document type 17 (a receipt for the proceeds of a sale) from NTA tax answer No.7141. Besides the amount received, whether an amount is stated and whether the receipt is in the course of business decide it.

```rule
rule 領収書の印紙税(receipt_stamp) v1
description "売上代金に係る金銭又は有価証券の受取書（第17号の1文書）の印紙税額。5万円未満と、営業に関しないものは非課税"

inputs
  受取金額(amount)       : money[円]  range >=0円 <=10000億円
  金額の記載あり(stated) : bool
  営業に関する(business) : bool

outputs
  印紙税額(tax) : money[円]  round down(1円)

table 税額(tax_table)  # 出典: 国税庁 タックスアンサー No.7141 印紙税額の一覧表（その2）第17号文書（令和8年4月1日現在法令等）
policy unique
| 営業に関する | 金額の記載あり | 受取金額             | -> 印紙税額(tax) : money[円] |
| false        | -              | -                    | 0円                          |  # 営業に関しないものは非課税
| true         | false          | -                    | 200円                        |  # 受取金額の記載のないもの
| true         | true           | <5万円               | 0円                          |  # 非課税
| true         | true           | >=5万円 <=100万円    | 200円                        |
| true         | true           | >100万円 <=200万円   | 400円                        |
| true         | true           | >200万円 <=300万円   | 600円                        |
| true         | true           | >300万円 <=500万円   | 1000円                       |
| true         | true           | >500万円 <=1000万円  | 2000円                       |
| true         | true           | >1000万円 <=2000万円 | 4000円                       |
| true         | true           | >2000万円 <=3000万円 | 6000円                       |
| true         | true           | >3000万円 <=5000万円 | 10000円                      |
| true         | true           | >5000万円 <=1億円    | 20000円                      |
| true         | true           | >1億円 <=2億円       | 40000円                      |
| true         | true           | >2億円 <=3億円       | 60000円                      |
| true         | true           | >3億円 <=5億円       | 100000円                     |
| true         | true           | >5億円 <=10億円      | 150000円                     |
| true         | true           | >10億円              | 200000円                     |

examples
| 受取金額  | 金額の記載あり | 営業に関する | -> 印紙税額 |
| 49999円   | true           | true         | 0円         |  # 5万円未満は非課税
| 50000円   | true           | true         | 200円       |
| 1000000円 | true           | true         | 200円       |  # 100万円以下
| 1000001円 | true           | true         | 400円       |  # 100万円を超え
| 0円       | false          | true         | 200円       |  # 記載のないもの
| 3000000円 | true           | false        | 0円         |  # 営業に関しないもの
```

**What this one shows**

- **Exempt is a row of 0 yen.** Under 50,000 yen, and receipts not in the course of business, are exempt, and the table holds that as `0円` rows: "not taxed" is an answer of the rule too.
- **A receipt with no amount stated does not look at the amount.** The row with `金額の記載あり` false has `-` in the amount column and is 200 yen whatever the amount. The checker proves that every combination of the three inputs hits exactly one row.

## The income-tax bracket table

The quick-calculation table of NTA tax answer No.2260. Each bracket of taxable income carries a rate and a deduction, and `taxable × rate − deduction` is the tax; the reconstruction surtax is 2.1% of it.

```rule
rule 所得税(income_tax) v1
description "所得税の速算表。課税される所得金額に税率を掛けて控除額を引き、復興特別所得税を上乗せする"

inputs
  課税所得(taxable) : money[円]  range >=1000円 <=100億円  # 1,000円未満の端数を切り捨てた後の金額。速算表は 1,000円 から始まる

outputs
  所得税(tax)            : money[円]  round down(1円)  # 課税所得が 1,000円 単位で税率が整数の % なので、端数は出ない
  復興特別所得税(surtax) : money[円]  round down(1円)  # 基準所得税額の 2.1%。1円未満の端数の扱いはこのページに無いので、切り捨てに仮置き

# 速算表は「1,000円 から 1,949,000円まで」「1,950,000円 から」のように書く。課税所得は 1,000円 単位なので、
# 各段の上端は次の段の始まりの手前と同じこと。ここではそのまま「次の段の始まり未満」と書く。
table 速算表(brackets)  # 出典: 国税庁 タックスアンサー No.2260 所得税の税率（令和8年4月1日現在法令等）
policy unique
| 課税所得                 | -> 税率(rate) : rate[step 1%] | 控除額(deduction) : money[円] |
| <1950000円               | 5%                            | 0円                           |
| >=1950000円 <3300000円   | 10%                           | 97500円                       |
| >=3300000円 <6950000円   | 20%                           | 427500円                      |
| >=6950000円 <9000000円   | 23%                           | 636000円                      |
| >=9000000円 <18000000円  | 33%                           | 1536000円                     |
| >=18000000円 <40000000円 | 40%                           | 2796000円                     |
| >=40000000円             | 45%                           | 4796000円                     |

define 所得税(tax) : money[円] = 課税所得 × 税率 − 控除額
define 復興特別所得税(surtax) : money[円] = 所得税 × 2.1%  # 出典: 同ページ「基準所得税額の2.1パーセント」

examples
| 課税所得  | -> 所得税 | 復興特別所得税 |
| 7000000円 | 974000円  | 20454円        |  # 同ページの計算例: 7,000,000円 × 0.23 − 636,000円 = 974,000円
| 1949000円 | 97450円   | 2046円         |  # 5% の段の上端。2,046.45円 → 切り捨て（仮置き）
| 1950000円 | 97500円   | 2047円         |  # 10% の段の下端。控除額のおかげで上の行と 50円 しか違わない
```

**What this one shows**

- **One table produces a rate and an amount at once.** The rate column is `rate[step 1%]`, the deduction column `money[円]`, and a `define` multiplies and subtracts. The page's own worked example (7,000,000 × 0.23 − 636,000 = 974,000 yen) is an `examples` row as it stands.
- **Bracket edges are written as "below the start of the next bracket".** The page says "from 1,000 to 1,949,000 yen" and "from 1,950,000 yen"; since taxable income is in units of 1,000 yen those are the same thing, and completeness over all the integers needs the form with no gap.
- **What the page does not say is marked as a placeholder.** How a fraction of a yen in the surtax is settled is not on this page. The rule says `round down` and keeps, in the comment beside the declaration, that the source is silent — which is what `rulec doc` shows the approver.

## Stamp duty on a contract, with a reduced rate that expires

The stamp duty on a contract for the transfer of real estate (document type 1). The standard amounts (No.7140) and the reduced amounts for contracts made up to 31 March 2027 (No.7108) sit in one table, with the date of the contract as an input.

```rule
rule 印紙税(stamp_duty) v1
description "不動産の譲渡に関する契約書（第1号文書）の印紙税額。記載された契約金額と作成日で決まり、令和9年3月31日までに作成されたものは軽減税率"

inputs
  契約金額(amount)       : money[円]  range >=0円 <=10000億円
  金額の記載あり(stated) : bool
  作成日(made)           : date  range >=2014-04-01 <=2030-12-31  # 軽減措置の始まり（平成26年4月1日）から

outputs
  印紙税額(tax) : money[円]  round down(1円)

define 軽減期間(reduced) : bool = 作成日 <= 2027-03-31  # 出典: No.7108。平成26年4月1日から令和9年3月31日までの間に作成される契約書。始まりは入力の範囲の下端

# 軽減税率は契約金額が 10万円 を超えるものだけ（No.7108）。10万円以下は期間によらず本則で、記載のないものは 200円。
table 税額(tax_table)  # 出典: 国税庁 タックスアンサー No.7140 印紙税額の一覧表（その1）第1号文書、No.7108 不動産の譲渡契約書等の印紙税の軽減措置（令和8年4月1日現在法令等）
policy unique
| 金額の記載あり | 軽減期間 | 契約金額             | -> 印紙税額(tax) : money[円] |
| false          | -        | -                    | 200円                        |  # 契約金額の記載のないもの
| true           | -        | <1万円               | 0円                          |  # 非課税
| true           | -        | >=1万円 <=10万円     | 200円                        |
| true           | true     | >10万円 <=50万円     | 200円                        |  # ここから軽減後の税額（No.7108）
| true           | true     | >50万円 <=100万円    | 500円                        |
| true           | true     | >100万円 <=500万円   | 1000円                       |
| true           | true     | >500万円 <=1000万円  | 5000円                       |
| true           | true     | >1000万円 <=5000万円 | 10000円                      |
| true           | true     | >5000万円 <=1億円    | 30000円                      |
| true           | true     | >1億円 <=5億円       | 60000円                      |
| true           | true     | >5億円 <=10億円      | 160000円                     |
| true           | true     | >10億円 <=50億円     | 320000円                     |
| true           | true     | >50億円              | 480000円                     |
| true           | false    | >10万円 <=50万円     | 400円                        |  # ここから本則の税額（No.7140）
| true           | false    | >50万円 <=100万円    | 1000円                       |
| true           | false    | >100万円 <=500万円   | 2000円                       |
| true           | false    | >500万円 <=1000万円  | 10000円                      |
| true           | false    | >1000万円 <=5000万円 | 20000円                      |
| true           | false    | >5000万円 <=1億円    | 60000円                      |
| true           | false    | >1億円 <=5億円       | 100000円                     |
| true           | false    | >5億円 <=10億円      | 200000円                     |
| true           | false    | >10億円 <=50億円     | 400000円                     |
| true           | false    | >50億円              | 600000円                     |

examples
| 契約金額   | 金額の記載あり | 作成日     | -> 印紙税額 |
| 30000000円 | true           | 2026-09-16 | 10000円     |  # 3,000万円の売買契約。軽減期間なので 1万円（本則は 2万円）
| 30000000円 | true           | 2027-04-01 | 20000円     |  # 軽減期間が終わった翌日
| 100000円   | true           | 2026-09-16 | 200円       |  # 10万円ちょうどは軽減の対象外で、本則も 200円
| 5000円     | true           | 2026-09-16 | 0円         |  # 1万円未満は非課税
| 0円        | false          | 2026-09-16 | 200円       |  # 契約金額の記載のないもの
```

**What this one shows**

- **A time-limited exception is a date definition and one column.** `define 軽減期間 = 作成日 <= 2027-03-31` goes into a column: `true` on the reduced rows, `false` on the standard ones, `-` where the period does not matter. Under `policy unique` every amount on every date is proved to hit exactly one row.
- **What the reduction does not cover, the standard rows take.** The reduction applies only above 100,000 yen, so the exempt row (under 10,000 yen) and the row up to 100,000 yen have `-` in the period column.
- **This rule found a defect in the generator.** A date literal inside a definition was generated as 0 in every language. The reference evaluator read the date, so the disagreement showed up in `rulec test`.

## The employees' pension grade table

The premium table for employees' pension from 日本年金機構 (fiscal 2026 edition): 32 grades, and a rate that is 18.3% for ordinary insured people but varies by fund for members of a pension fund, so the rate is an input.

```rule
rule 厚生年金保険料(pension_premium) v1
description "厚生年金保険料。報酬月額から標準報酬月額を引き、料率を掛けて折半し、給与から控除する額と現金で納める額を出す"

source 機構 = file "sources/nenkin/R08ryougaku.xlsx" sha256:462a199ad4f3b69c  # 日本年金機構 令和2年9月分（10月納付分）からの厚生年金保険料額表（令和8年度版）
  表1 sha256:09a23ce9620373c4

inputs
  報酬月額(monthly) : money[円]  range >=0円 <=1000万円
  料率(rate)        : rate[step 0.1%]  range >=0% <=30%   # 一般・坑内員・船員は 18.3%。厚生年金基金の加入員は基金ごとに 13.3%〜15.9% なので入力にする

outputs
  標準報酬月額(std)  : money[円]  round down(1円)
  給与控除額(deduct) : money[円]  round half_down(1円)  # 出典: 保険料額表の注記①。給与から控除するとき、50銭以下は切り捨て、50銭を超えれば切り上げ
  現金納付額(cash)   : money[円]  round half_up(1円)    # 出典: 保険料額表の注記②。現金で納めるとき、50銭未満は切り捨て、50銭以上は切り上げ

table 等級(grade)  @機構 表1  # 保険料額表の 1〜32 等級をそのまま写した
policy unique
| 報酬月額             | -> 標準報酬月額(std) : money[円] |
| <93000円             | 88000円                          |
| >=93000円 <101000円  | 98000円                          |
| >=101000円 <107000円 | 104000円                         |
| >=107000円 <114000円 | 110000円                         |
| >=114000円 <122000円 | 118000円                         |
| >=122000円 <130000円 | 126000円                         |
| >=130000円 <138000円 | 134000円                         |
| >=138000円 <146000円 | 142000円                         |
| >=146000円 <155000円 | 150000円                         |
| >=155000円 <165000円 | 160000円                         |
| >=165000円 <175000円 | 170000円                         |
| >=175000円 <185000円 | 180000円                         |
| >=185000円 <195000円 | 190000円                         |
| >=195000円 <210000円 | 200000円                         |
| >=210000円 <230000円 | 220000円                         |
| >=230000円 <250000円 | 240000円                         |
| >=250000円 <270000円 | 260000円                         |
| >=270000円 <290000円 | 280000円                         |
| >=290000円 <310000円 | 300000円                         |
| >=310000円 <330000円 | 320000円                         |
| >=330000円 <350000円 | 340000円                         |
| >=350000円 <370000円 | 360000円                         |
| >=370000円 <395000円 | 380000円                         |
| >=395000円 <425000円 | 410000円                         |
| >=425000円 <455000円 | 440000円                         |
| >=455000円 <485000円 | 470000円                         |
| >=485000円 <515000円 | 500000円                         |
| >=515000円 <545000円 | 530000円                         |
| >=545000円 <575000円 | 560000円                         |
| >=575000円 <605000円 | 590000円                         |
| >=605000円 <635000円 | 620000円                         |
| >=635000円           | 650000円                         |

define 折半額(half) : money[円] = 標準報酬月額 × 料率 ÷ 2  # 全額の折半。円未満の端数は上の二つの出力の丸めで決まる
define 給与控除額(deduct) : money[円] = 折半額
define 現金納付額(cash) : money[円] = 折半額

examples
| 報酬月額 | 料率  | -> 標準報酬月額 | 給与控除額 | 現金納付額 |
| 90000円  | 18.3% | 88000円         | 8052円     | 8052円     |  # 表の 1 等級。全額 16,104.00円、折半額 8,052.00円
| 250000円 | 18.3% | 260000円        | 23790円    | 23790円    |  # 表の 17 等級。250,000円以上 270,000円未満
| 700000円 | 18.3% | 650000円        | 59475円    | 59475円    |  # 表の 32 等級（上限）。折半額 59,475.00円
```

**What this one shows**

- **The same shape as the health-insurance rule.** Fifty grades become thirty-two and the ceiling is 650,000 yen; the halving and the two ways of settling the sen are unchanged. Rules of one shape transcribe into rules of one shape.
- **Here the two ways agree.** 18.3% of a standard remuneration is always an even number of yen, so the half has no fraction. The rule states both roundings; the `examples` show that at this rate the difference never appears.
- **All 32 printed grades are held to the rule.** The printed halves are transcribed into records (`tests/oracle/`), and a test replays the rule over them and requires every one to agree.
- **And the table is held to the workbook it came from.** The `source` line points at 日本年金機構's own `.xlsx` and `@機構 表1` cites its first sheet; `rulec source fetch` takes that sheet out and keeps it beside the rule, and every `rulec check` then requires **each of the 32 standard remunerations to be a value that copy shows** (E116). Write `470000円` as `480000円` and it fails there alone — on the rounding grid, no gap, no overlap.

## A premium table, with two ways to settle the sen

The 協会けんぽ premium table (Tokyo branch, from March 2026). Monthly pay picks one of 50 grades of standard remuneration, the rate is applied and the amount halved. Fractions of a yen are settled two different ways — one when the premium is deducted from salary, another when it is paid in cash — and transcribing this table is what put `half_down` into the language.

```rule
rule 健康保険料(kenpo_premium) v1
description "協会けんぽの健康保険料。報酬月額から標準報酬月額を引き、料率を掛けて折半し、給与から控除する額と現金で納める額を出す"

inputs
  報酬月額(monthly)       : money[円]  range >=0円 <=1000万円
  健康保険料率(rate)      : rate[step 0.01%]  range >=0% <=20%   # 都道府県ごと、年度ごとに変わるので入力にする
  介護保険料率(care_rate) : rate[step 0.01%]  range >=0% <=5%    # 介護保険第2号被保険者（40〜64歳）に加わる分
  介護該当(care)          : bool

outputs
  標準報酬月額(std)  : money[円]  round down(1円)
  給与控除額(deduct) : money[円]  round half_down(1円)  # 出典: 保険料額表の注記①。給与から控除するとき、50銭以下は切り捨て、50銭を超えれば切り上げ
  現金納付額(cash)   : money[円]  round half_up(1円)    # 出典: 保険料額表の注記②。現金で納めるとき、50銭未満は切り捨て、50銭以上は切り上げ

derive 合算率(both) : rate[step 0.01%] = 健康保険料率 + 介護保険料率  range >=0% <=25%

table 等級(grade)  # 出典: 全国健康保険協会 令和8年3月分（4月納付分）からの健康保険・厚生年金保険の保険料額表（東京支部）
policy unique
| 報酬月額               | -> 標準報酬月額(std) : money[円] |
| <63000円               | 58000円                          |
| >=63000円 <73000円     | 68000円                          |
| >=73000円 <83000円     | 78000円                          |
| >=83000円 <93000円     | 88000円                          |
| >=93000円 <101000円    | 98000円                          |
| >=101000円 <107000円   | 104000円                         |
| >=107000円 <114000円   | 110000円                         |
| >=114000円 <122000円   | 118000円                         |
| >=122000円 <130000円   | 126000円                         |
| >=130000円 <138000円   | 134000円                         |
| >=138000円 <146000円   | 142000円                         |
| >=146000円 <155000円   | 150000円                         |
| >=155000円 <165000円   | 160000円                         |
| >=165000円 <175000円   | 170000円                         |
| >=175000円 <185000円   | 180000円                         |
| >=185000円 <195000円   | 190000円                         |
| >=195000円 <210000円   | 200000円                         |
| >=210000円 <230000円   | 220000円                         |
| >=230000円 <250000円   | 240000円                         |
| >=250000円 <270000円   | 260000円                         |
| >=270000円 <290000円   | 280000円                         |
| >=290000円 <310000円   | 300000円                         |
| >=310000円 <330000円   | 320000円                         |
| >=330000円 <350000円   | 340000円                         |
| >=350000円 <370000円   | 360000円                         |
| >=370000円 <395000円   | 380000円                         |
| >=395000円 <425000円   | 410000円                         |
| >=425000円 <455000円   | 440000円                         |
| >=455000円 <485000円   | 470000円                         |
| >=485000円 <515000円   | 500000円                         |
| >=515000円 <545000円   | 530000円                         |
| >=545000円 <575000円   | 560000円                         |
| >=575000円 <605000円   | 590000円                         |
| >=605000円 <635000円   | 620000円                         |
| >=635000円 <665000円   | 650000円                         |
| >=665000円 <695000円   | 680000円                         |
| >=695000円 <730000円   | 710000円                         |
| >=730000円 <770000円   | 750000円                         |
| >=770000円 <810000円   | 790000円                         |
| >=810000円 <855000円   | 830000円                         |
| >=855000円 <905000円   | 880000円                         |
| >=905000円 <955000円   | 930000円                         |
| >=955000円 <1005000円  | 980000円                         |
| >=1005000円 <1055000円 | 1030000円                        |
| >=1055000円 <1115000円 | 1090000円                        |
| >=1115000円 <1175000円 | 1150000円                        |
| >=1175000円 <1235000円 | 1210000円                        |
| >=1235000円 <1295000円 | 1270000円                        |
| >=1295000円 <1355000円 | 1330000円                        |
| >=1355000円            | 1390000円                        |

table 適用料率(applied)  # 介護保険第2号被保険者は、健康保険料率に介護保険料率を足した率になる
policy unique
| 介護該当 | -> 料率(applied_rate) : rate[step 0.01%] |
| true     | 合算率                                   |
| false    | 健康保険料率                             |

define 折半額(half) : money[円] = 標準報酬月額 × 料率 ÷ 2  # 全額の折半。円未満の端数は上の二つの出力の丸めで決まる
define 給与控除額(deduct) : money[円] = 折半額
define 現金納付額(cash) : money[円] = 折半額

examples
| 報酬月額 | 健康保険料率 | 介護保険料率 | 介護該当 | -> 標準報酬月額 | 給与控除額 | 現金納付額 |
| 60000円  | 9.85%        | 1.62%        | false    | 58000円         | 2856円     | 2857円     |  # 折半額 2,856.5円。給与控除は切り捨て、現金納付は切り上げ
| 134000円 | 9.85%        | 1.62%        | false    | 134000円        | 6599円     | 6600円     |  # 折半額 6,599.5円
| 134000円 | 9.85%        | 1.62%        | true     | 134000円        | 7685円     | 7685円     |  # 折半額 7,684.9円。どちらも切り上げ
| 300000円 | 9.85%        | 1.62%        | true     | 300000円        | 17205円    | 17205円    |  # 折半額 17,205.0円
```

**What this one shows**

- **Two outputs from one halved amount, rounded two ways.** The table's notes say: deducted from salary, half a yen or less is dropped and more than half is carried up; paid in cash, less than half is dropped and half or more is carried up. The second is `half_up`; the first is `half_down`. At the grade whose half is 6,599.5 yen the two outputs differ by one yen.
- **The rates are inputs.** They change by prefecture and by year; baking them into the rule would mean rewriting the table at every revision. A `derive` adds the care-insurance rate to the health-insurance rate, and a table picks which applies by whether the person is a category-2 care insured.
- **Every grade is held to the printed table.** The printed halves are transcribed into records (`tests/oracle/`), and a test replays the rule over all 100 of them and requires every one to agree.

## A main rule and a reduced rate as two tables, held to their sources

The stamp duty rule above, split into the main table (Appendix Table 1 of the Stamp Tax Act) and the reduced-rate table (Article 91 of the Special Taxation Measures Act), with the exemption as a clause. Each table cites its own source, and each source is held to the digest of a copy of the text fetched from e-Gov, the Japanese government's statute database.

```rule
rule 印紙税の本則と軽減(stamp_duty_split) v1
description "不動産の譲渡に関する契約書（第1号文書）の印紙税額。本則の表に、令和9年3月31日までの軽減税率の表が優先する"

source 法 = law "342AC0000000023" asof 2026-04-01
  別表第一 sha256:0ba69792e960021e
source 措置法 = law "332AC0000000026" asof 2026-04-01
  第91条 sha256:85faf53f6f6e8196

inputs
  契約金額(amount)       : money[円]  range >=0円 <=10000億円
  金額の記載あり(stated) : bool
  作成日(made)           : date  range >=2014-04-01 <=2030-12-31

outputs
  印紙税額(tax) : money[円]  round down(1円)

define 軽減期間(reduced) : bool = 作成日 <= 2027-03-31  @措置法 第91条

table 本則(base)  @法 別表第一  # 第1号文書の欄
policy unique
         | 金額の記載あり | 契約金額             | -> 印紙税額(tax) : money[円] |
記載なし | false          | -                    | 200円                        |
r3       | true           | >=1万円 <=10万円     | 200円                        |
r4       | true           | >10万円 <=50万円     | 400円                        |
r5       | true           | >50万円 <=100万円    | 1000円                       |
r6       | true           | >100万円 <=500万円   | 2000円                       |
r7       | true           | >500万円 <=1000万円  | 10000円                      |
r8       | true           | >1000万円 <=5000万円 | 20000円                      |
r9       | true           | >5000万円 <=1億円    | 60000円                      |
r10      | true           | >1億円 <=5億円       | 100000円                     |
r11      | true           | >5億円 <=10億円      | 200000円                     |
r12      | true           | >10億円 <=50億円     | 400000円                     |
r13      | true           | >50億円              | 600000円                     |

clause 非課税(exempt) -> 印紙税額  @法 別表第一  # 第1号文書の非課税物件の欄（記載された契約金額が1万円未満のもの）
  when 金額の記載あり true and 契約金額 <1万円
  then 0円

table 軽減(reduced_rate)  @措置法 第91条
policy unique
overrides 本則
| 軽減期間 | 金額の記載あり | 契約金額             | -> 印紙税額 |
| true     | true           | >10万円 <=50万円     | 200円       |
| true     | true           | >50万円 <=100万円    | 500円       |
| true     | true           | >100万円 <=500万円   | 1000円      |
| true     | true           | >500万円 <=1000万円  | 5000円      |
| true     | true           | >1000万円 <=5000万円 | 10000円     |
| true     | true           | >5000万円 <=1億円    | 30000円     |
| true     | true           | >1億円 <=5億円       | 60000円     |
| true     | true           | >5億円 <=10億円      | 160000円    |
| true     | true           | >10億円 <=50億円     | 320000円    |
| true     | true           | >50億円              | 480000円    |

examples
| 契約金額   | 金額の記載あり | 作成日     | -> 印紙税額 |
| 30000000円 | true           | 2026-09-16 | 10000円     |
| 30000000円 | true           | 2027-04-01 | 20000円     |
| 100000円   | true           | 2026-09-16 | 200円       |
| 5000円     | true           | 2026-09-16 | 0円         |
| 0円        | false          | 2026-09-16 | 200円       |
```

**What this one shows**

- **`overrides 本則` makes the exception take precedence over the main rule.** Instead of adding a `軽減期間` column to one table, the tables follow the documents, and one line says which wins. The checks judge completeness and overlaps over the two together.
- **The exemption is a `clause`.** In the appendix table it sits in the column of exempt documents, not in the table of taxable ones, so it is written as a sentence rather than a row.
- **`source` and `@` hold the rule to its documents.** `rulec source fetch` brings copies of the appendix table and Article 91 from e-Gov, `rulec source pin` writes their digests. When the text changes, the check stops and names the tables that cite that place (E038).
- **Row labels** (`r1` …) are the names the trace reports and the names `overrides 本則:r3` points at.

## A proviso written as a sentence

A tariff table decides the base fee, and two clauses decide the shipping fee: the main text ("regular") and the proviso that makes a member's order of 3,900 yen or more free. The proviso's conditions do not line up as columns, so it is a `clause`, not a table.

```rule
rule 送料のただし書(shipping_proviso) v1
description "通常便の送料。運賃表で基本運賃を決め、会員の 3,900円 以上の注文を無料にするただし書が本則に優先する（DESIGN.md §15.67 のスケッチ）"

import std/都道府県

enum サイズ区分(size_class) = S60(s60) | S80(s80)
group 遠隔地 = 北海道, 沖縄県

inputs
  あて先(dest)    : 都道府県
  サイズ(size)    : サイズ区分
  会員(member)    : bool
  注文金額(total) : money[円]  range >=0円 <=1000万円

outputs
  送料(fee) : money[円]  round up(10円)

table 運賃表(fee_table)  # 出典: 基本運賃表（スケッチ）別表第一
policy unique
| あて先      | サイズ | -> 基本運賃(base) : money[円] |
| 遠隔地      | S60    | 1150円                        |
| 遠隔地      | S80    | 1400円                        |
| not: 遠隔地 | S60    | 820円                         |
| not: 遠隔地 | S80    | 1050円                        |

clause 通常(regular) -> 送料  # 出典: 第3条第1項（本文）
  when always
  then 基本運賃

clause 無料(free) -> 送料  # 出典: 第3条第2項ただし書
  when 注文金額 >=3900円 and 会員 true
  then 0円
  overrides 通常

examples
| あて先 | サイズ | 会員  | 注文金額 | -> 送料 |
| 北海道 | S60    | true  | 3900円   | 0円     |
| 北海道 | S60    | true  | 3899円   | 1150円  |
| 東京都 | S80    | false | 10000円  | 1050円  |
```

**What this one shows**

- **A `clause` is a one-row table.** The condition under `when`, the value under `then`; checked, generated and traced like a table, firing as `{"table":"無料","row":1}`.
- **`overrides 通常` makes the proviso take precedence over the main text.** The approver's page says "clause 無料 takes precedence over clause 通常; in the 1 pair that meets, its row lies inside the other's (an exception)".
- **A group without an alias** (`group 遠隔地 = 北海道, 沖縄県`) is allowed; the generated identifiers number it.

## The rule the next one applies

The rule applied by the next example. Years of service and the reason for leaving decide the number of months paid, and a clause reducing the allowance on voluntary resignation takes precedence over the main rule. It is a sketch from the design document, not a real statute.

```rule
rule 退職手当(retirement_allowance) v1
description "退職手当の本則。勤続年数で支給月数を決め、自己都合退職の減額が本則に優先する（DESIGN.md §15.69 のスケッチ）"

enum 事由(reason_kind) = 定年(retirement_age) | 自己都合(voluntary) | 死亡(death)

inputs
  勤続年数(years)  : number    range >=1 <=40
  退職事由(reason) : 事由
  基本給(base_pay) : money[円]  range >=10万円 <=100万円

outputs
  手当(allowance) : money[円]  round down(1円)

table 支給表(schedule)  # 出典: 第20条第1項（スケッチ）
policy unique
     | 勤続年数 | 退職事由       | -> 支給月数(months) : number |
短期 | <10      | 定年, 自己都合 | 5                            |
中期 | >=10 <25 | 定年, 自己都合 | 20                           |
長期 | >=25     | 定年, 自己都合 | 40                           |
死亡 | -        | 死亡           | 40                           |

define 満額(full) : money[円] = 基本給 × 支給月数
define 減額後(reduced) : money[円] = 満額 × 80%

clause 本則(main) -> 手当  # 出典: 第20条第1項（スケッチ）
  when always
  then 満額

clause 減額(reduction) -> 手当  # 出典: 第20条第2項（スケッチ）
  when 退職事由 自己都合
  then 減額後
  overrides 本則

examples
| 勤続年数 | 退職事由 | 基本給 | -> 手当  |
| 3        | 定年     | 30万円 | 150万円  |
| 3        | 自己都合 | 30万円 | 120万円  |
| 30       | 死亡     | 30万円 | 1200万円 |
```

**What this one shows**

- **It is checked and generated on its own.** The rule that applies it writes this file's digest in its heading and is held to it.
- **The `減額` clause can be left out by the applying rule with `except`** — "Article 20 (excluding paragraph 2) applies".

## Applying another rule with its terms read differently

The retirement allowance rule above, applied to part-time staff: "years of service" is read as "period in office", "reason for leaving" as "how the term ended", and the reduction clause is not applied.

```rule
rule 非常勤退職手当(part_time_allowance) v1
description "非常勤職員の退職手当。退職手当の規定を、在職期間と任期終了事由に読み替えて準用し、減額の規定は準用しない（DESIGN.md §15.69 のスケッチ）"

enum 終了事由(end_kind) = 任期満了(term_end) | 辞職(resignation)

inputs
  在職期間(tenure)       : number    range >=1 <=3
  任期終了事由(end_reason) : 終了事由
  報酬月額(monthly_pay)  : money[円]  range >=10万円 <=50万円

outputs
  非常勤手当(allowance) : money[円]  round down(1円)

apply 退職手当(retirement) = "退職手当.rule" sha256:fb21d081458c197e  # 出典: 第31条（スケッチ）
  勤続年数 = 在職期間
  退職事由 = 任期終了事由 with 任期満了 -> 定年, 辞職 -> 自己都合
  基本給 = 報酬月額
  except 減額
  手当 -> 非常勤手当

examples
| 在職期間 | 任期終了事由 | 報酬月額 | -> 非常勤手当 |
| 3        | 任期満了     | 30万円   | 150万円       |
| 3        | 辞職         | 30万円   | 150万円       |
```

**What this one shows**

- **A substitution is `<input of the applied rule> = <value of this rule>`.** Two enums are matched value by value: `with 任期満了 -> 定年, 辞職 -> 自己都合`.
- **The check proves that what is passed stays inside the applied rule's ranges (E043).** The period in office is 1 to 3 years, inside the 1 to 40 of years of service; declared from 0, the check stops with that value as the example.
- **The applied rule's tables are expanded into this rule, checked and generated with it.** The trace reports the original table's name: `{"table":"退職手当:支給表","row":1,"label":"短期"}`. The rows for ten years of service and more are never reached here; they are not errors, and the approver's page lists them as unused by this apply.
- **When the applied rule changes, E040 stops the check.** `rulec diff` shows how many answers move and by how much; once accepted, `rulec source pin` writes the new digest.

## A temperature and a volume decide the label

A transcription of a food storage standard. A temperature in degrees Celsius, a volume in millilitres, and a column that is allowed to hold "not decided yet", all in one rule.

```rule
rule 保存基準(storage) v1
description "食品の保存温度と容量から、貼る表示と取るべき措置を決める（食品衛生法の規格基準のスケッチ）"

enum 区分(kind) = 冷凍(frozen) | 冷蔵(chilled) | 常温(ambient)
enum 措置(action) = 適合(ok) | 要冷却(cool) | 廃棄(discard)

inputs
  保存温度(temp)   : temperature[℃]  range >=-30℃ <=40℃
  容量(volume)     : volume[mL]       range >=0mL <=5000mL
  表示区分(label)  : 区分
  再検区分(recheck) : 区分?

outputs
  判定(verdict) : 措置
  表示(text)    : string

table 温度判定(temp_of)
policy unique
| 表示区分 | 保存温度   | -> 適温(in_range) : bool |
| 冷凍     | <=-15℃     | true                     |
| 冷凍     | >-15℃      | false                    |
| 冷蔵     | >=0℃ <=10℃ | true                     |
| 冷蔵     | <0℃        | false                    |
| 冷蔵     | >10℃       | false                    |
| 常温     | -          | true                     |

table 再検判定(recheck_of)
policy unique
| 再検区分 | 容量     | -> 小分け(small) : bool |
| none     | -        | false                   |
| 冷凍     | -        | true                    |
| 冷蔵     | <=1000mL | true                    |
| 冷蔵     | >1000mL  | false                   |
| 常温     | -        | false                   |

table 措置判定(action_of)
policy first
| 適温  | 小分け | 保存温度 | -> 判定(verdict) : 措置 | 表示(text) : string |
| true  | -      | -        | 適合                    | "適合"              |
| false | true   | -        | 要冷却                  | "要冷却・小分け"    |
| false | -      | >25℃     | 廃棄                    | "廃棄"              |
| false | -      | -        | 要冷却                  | "要冷却"            |

examples
| 保存温度 | 容量   | 表示区分 | 再検区分 | -> 判定 | 表示             |
| -20℃     | 500mL  | 冷凍     | none     | 適合    | "適合"           |
| -10℃     | 500mL  | 冷凍     | 冷凍     | 要冷却  | "要冷却・小分け" |
| 5℃       | 1000mL | 冷蔵     | none     | 適合    | "適合"           |
| 30℃      | 2000mL | 冷蔵     | 冷蔵     | 廃棄    | "廃棄"           |
| 15℃      | 500mL  | 冷蔵     | none     | 要冷却  | "要冷却"         |
```

**What this one shows**

- **A temperature is a scale to compare against, and nothing more.** A ℃ has a displaced zero, so it can be neither added nor doubled; comparison and `range` are all there is, and `気温 - 気温` stops at E048.
- **`区分?` is a column that may hold nothing.** Only the cell `none` accepts it, and it never appears in an expression — null is kept out of arithmetic by making the table branch on it.
- **An output may be a string**, such as the label a person reads. A string cannot be a table's *input* column (E110): a value that decides a branch belongs in an enum.

## Area, noise and overtime decide the measure and the cost

A sketch of Japan's office hygiene rules and its noise-exposure guidance. Three dimensions — area, sound and time — and a declared relation between two inputs.

```rule
rule 事務所の衛生基準(office_standard) v1
description "床面積と天井高から気積を、騒音と残業時間と合わせて、事業者が取る措置と負担額を決める（事務所衛生基準規則と騒音障害防止のスケッチ）"

enum 措置(action) = 適合(ok) | 要改善(improve) | 使用停止(stop)

inputs
  在籍(people)   : number          range >=1 <=200
  床面積(floor)  : area[m2]        range >=1m2 <=2000m2
  占有面積(used) : area[m2]        range >=1m2 <=2000m2
  騒音(noise)    : sound[dB]       range >=0dB <=130dB
  残業(overtime) : duration[h]     range >=0h <=200h

# 占有している面積が床面積を超えることはない。検査はこの外側に行を求めない
constraint 占有面積 <= 床面積

outputs
  判定(verdict)  : 措置
  負担額(cost)   : money[円, incl_tax]  round half_even(100円)

derive 余裕(spare) : area[m2] = 床面積 - 占有面積  range >=-1999m2 <=1999m2

table 広さ判定(space_of)
policy first
| 余裕  | -> 狭い(tight) : bool |
| <10m2 | true                  |
| -     | false                 |

table 措置判定(action_of)
policy first
| 騒音   | 残業 | 狭い | 在籍 | -> 判定(verdict) : 措置 | 率(rate) : rate[step 10%] |
| >=90dB | -    | -    | -    | 使用停止                | 100%                      |
| >=85dB | -    | -    | -    | 要改善                  | 50%                       |
| -      | >45h | -    | -    | 要改善                  | 50%                       |
| -      | -    | true | >=50 | 使用停止                | 100%                      |
| -      | -    | true | -    | 要改善                  | 30%                       |
| -      | -    | -    | -    | 適合                    | 0%                        |

define 負担額(cost) : money[円, incl_tax] = 10000円 × 率

examples
| 在籍 | 床面積 | 占有面積 | 騒音 | 残業 | -> 判定  | 負担額  |
| 10   | 100m2  | 50m2     | 60dB | 10h  | 適合     | 0円     |
| 10   | 100m2  | 95m2     | 60dB | 10h  | 要改善   | 3000円  |
| 10   | 100m2  | 50m2     | 86dB | 10h  | 要改善   | 5000円  |
| 10   | 100m2  | 50m2     | 95dB | 10h  | 使用停止 | 10000円 |
| 10   | 100m2  | 50m2     | 60dB | 60h  | 要改善   | 5000円  |
```

**What this one shows**

- **`constraint 占有面積 <= 床面積` is a relation the caller guarantees.** Completeness then demands no row outside it, every witness becomes a case somebody could really send, and the generated code refuses a violating input at the door.
- **An area is a dimension of its own, not the product of two lengths.** `縦 × 横` is E103: this tool does no dimensional analysis, and will not invent a dimension to hold a product.
- **A sound level is another scale to compare against.** It is logarithmic, so adding two decibels is not two sounds' worth.
- **`round half_even` is one of the five roundings**, the one that sends a tie to the even side — the direction accounting usually asks for.
