# 例で見る

ここにあるのは全部、**このリポジトリのテストが毎回走らせている規則**です。`rulec check` を通り、書いてある例が実行され、参照評価器と生成したどの言語も同じ答えを返すことまで確かめられています。そのままコピーして動かせます。

小さいものから順に並べてあります。

## 日付で区分を返す

いちばん小さい例です。入力は注文日ひとつ、出力は期間の区分ひとつ。行は隣り合う日付の範囲で、隙間なく敷き詰めてあります。

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

**この例が見せていること**

- 日付は**比較と範囲だけ**です。足し算も引き算もありません。
- `<=2026-03-31` と `>=2026-04-01` が隣り合っていることを検査が知っています。内部で日付を通算日として持っているからで、`20260331` のような数で持つと月末と翌月初のあいだに実在しない隙間ができ、偽のエラーが出ます。
- 出力が金額ではないので、`round` は要りません。

## 条件が三つ絡む送料

設計のスケッチをそのまま規則にしたものです。真偽の定義を表の列に置く形、率の出力、`default` の印、`policy first` の隠れが一度に出てきます。

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

**この例が見せていること**

- `define … : bool` は**表の列に置ける真偽の名前**です。条件に名前が付くので、表のほうが読みやすくなります。
- `default` は「この値に専用の行は要らない」という宣言です。付けないと「どの行にも現れません」と警告されます。
- `policy first` なので、上の行が下の行を隠します。それが階段として自然な形かどうかを、検査が三つに分けて数えます。

## 実在の運賃表を写す

日本郵便の基本運賃表（東京発）です。47 都道府県 × 7 サイズ = 329 通りを、6 つのグループで 42 行に畳んでいます。**この道具の最初の実用価値がここにあります** — 県をひとつ書き落とすと、走らせる前に、その県を名指しして止まります。

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

**この例が見せていること**

- `group` は列挙の一部に名前を付けたものです。検査のときは必ずもとの値に展開されるので、**グループの分け方が 47 県の過不足ない分割になっているか**まで検査されます。
- `import std/都道府県` で 47 値が入ります（ASCII の別名つき）。
- 行が 42 本あっても、`policy unique` なので**並べ替えても意味が変わらない**ことが証明されています。

## 出力が二つある

クーポン 1 枚について、使えるかどうか（可否）と、いくら引くか（素割引）を同時に返します。重ね掛けの順序と反復は呼び出し側の仕事です。

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

**この例が見せていること**

- **出力は二つ以上書けます。** 二つの表が一つずつ埋め、どちらにも `round` が別々に効きます。
- 出力のセルには**値か名前を一つ**だけ書けます。計算は `define` に出します — 表には分岐だけを残すためです。
- 一つ前の表の出力（`可否`）を、次の表の列に使っています。上から下への一方通行なので、依存はいつでも目で追えます。

## 可否と金額を一度に

同じ題材を別の形で書いたものです。導出（`derive`）で残額を出し、それを表の列に置いています。

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

**この例が見せていること**

- `derive` は**入力どうしの足し算・引き算だけ**でできた、名前の付いた金額です。数量のまま表の列に置ける唯一の途中の値で、`range` の宣言が要ります。
- 宣言した範囲が、実際に起こりうる値を含んでいないと E112 で止まります。範囲は検査が使う「全体集合」だからです。

## 検査が決められなかったとき

二つの導出が同じ入力を共有しているため、二つの行が同時に当てはまりうるかを検査が決められません。黙って通すことも、嘘のエラーを出すこともしません。**警告を出し、生成コードに実行時のガードを入れます。**

```rule
rule クーポン併用(coupon_stack) v1
description "二つの導出が入力を共有する 一意 の表。列ごとのふるいには重なって見え、消去が起きないと決める題材（§6.2、§15.126）"

enum 判定(verdict) = 対象外(no) default | 対象(yes)

inputs
  合計(total)   : money[円,incl_tax]  range >=0円 <=100万円
  割引A(disc_a) : money[円,incl_tax]  range >=0円 <=10万円
  割引B(disc_b) : money[円,incl_tax]  range >=0円 <=10万円

outputs
  併用可否(verdict) : 判定

# 割引B は 0 円以上なので、残高B は定義から必ず 残高A 以下になる。
# つまり 残高A <= 1000円 のとき 残高B が 3980円 に届くことはない。
# 検査は導出を一本ずつ独立に見るので、この結びつきが見えない。下の 行1 と 行2 は
# そこでは重なって見え、Fourier-Motzkin 消去が起きないと決める（§15.126）。
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

**この例が見せていること**

- W114 は「証明できなかった」という警告です。重なりが**ある**とも**ない**とも言っていません。
- 生成コードには、万一その条件に当てはまる入力が来たとき、黙って先の行を選ばずにエラーを返すガードが入ります。**このガードが動いたなら、重なりは本当にあったということです。**
- 静的に決められないことを実行時に持ち越す唯一の場所です。

## 答えが順序

クーポン二枚のどちらを先に適用するかを返します。並べ替えそのものは呼び出し側がしますが、**比較の基準は規則の中にあります** — そこが承認の対象だからです。

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

**この例が見せていること**

- 返せる答えは金額・可否・区分・順序の四種で、これはその四つめです。
- 入力は「二枚ぶんの属性」で、出力は `先` か `後`。一度の判定に収まる形にしてあります。
- 日付どうしの比較が出てきます。

## 按分を、一件ずつに割る

一括の値引きを明細に配り切ります。**1 明細 = 1 回の判定**にしてあり、反復と残額の持ち回りは呼び出し側です。呼ぶ側は明細を順に回して残りを引くだけで、**合計は構成上ぴったり合います**（2,000 通りの明細で確かめました）。

```rule
rule 値引の充当(discount_fill) v1
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

**この例が見せていること**

- 「配り方」を決めるのが規則、「配って回る」のが呼び出し側、という切り分けです。
- `min` が使えます。上限で頭打ちにする形は、これで一行に収まります。
- 定価の比で割り付ける按分は、この形ではなく `allocate` で書きます（下の「比で配る按分」）。

## 1% より細かい率

契約ごとに決まる手数料率から、取引一件の手数料を出します。率の刻みが 0.1% なので、`0.5%` のような値を書けます。

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

**この例が見せていること**

- `rate[step 0.1%]` は「実行時の値は 0.1% の整数倍」という宣言です。0.5% は 5 刻み、10% は 100 刻みとして運ばれます。
- **刻みの間の値は書けません。** `rate[step 0.1%]` の列に `0.05%` と書くと E114 で止まります。黙って近い刻みに寄せると、表で読める境界と生成コードの境界が食い違うからです。
- 率のまま計算して、最後に一度だけ丸めます。途中で丸めないのは、丸め方が業務の判断だからです。

## 点数を合算して、達成率でランクを付ける

お金がまったく出てこない例です。四つの評価項目を重み付きで足し、満点に対する達成率でランクを決めます。「表で書かれてはいないが、書き出せば表と少しの式で足りる」ルールの典型です。

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

**この例が見せていること**

- `derive` は**入力どうしの足し算・引き算と整数倍**でできた名前です。重み付きの合計はこれで書けます。
- `define 達成率 : rate = 合計点 ÷ 50` と宣言すると、表のセルに `>=90%` と**割合のまま**書けます。無次元の値を率と呼ぶか数と呼ぶかは、宣言する側が決めます。
- **実行時に割り算は起きません。** `>=90%` は生成コードでは `合計点 >= 45` になります。満点が定数なので、境界が定数に畳めるからです。逆に**満点が入力だと書けません** — 変数で割ることになり、E115 で止まります（割合そのものを `rate` の入力で受け取ってください）。

## 単位のない数を返す

100 円につき 1 点。金額を金額で割ると単位が消えて、`number`（単位のない整数）になります。

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

**この例が見せていること**

- `number` は件数・日数・点数のような、**単位を持たない整数**のための型です。
- `税込金額 ÷ 100円` の割り算は**整数を割りません**。内部の刻みを 100 倍するだけなので、1050 円は 10.5 点のまま最後まで運ばれ、`round down(1)` が一度だけ 10 点に落とします。Python の `//` や Go の `/` の切り捨ての向きの違いが入り込む余地がありません。
- 割り算は**定数でだけ**書けます。割る数が業務データなら、それは率か定数表として表に現れるはずです。

## 表を三段重ねて、出力を二つ返す

重量から重さ区分、重さ区分と会員区分から帯、帯と支払額から送料と倍率。前の表が出した値が、そのまま次の表の列になります。返るのは請求額とポイントの二つです。

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

**この例が見せていること**

- **前の表の出力は、後の表の列にそのまま書けます。** 段数に上限はありません（検査の予算を超えたら E109 で止まります）。`rulec check` が落ちたときの「当てはまった行」も、`表 重さ判定 行2 / 表 帯判定 行4 / 表 送料表 行4` と段の数だけ出ます。
- **一つの表が出力列を複数持てます。** `送料表` は `送料` と `倍率` を同時に出し、下の `define` がその率を使います。率の刻み（`step 10%`）は列に入っても保たれるので、`基本点 × 倍率` は最後に一度だけ丸められます。
- **`derive` が列になります。** `支払額 = 商品合計 − 値引` を宣言してあるので、「値引き後の金額で判定する」が一本の式ではなく `送料表` の一つの列になります。
- **`result` が組み立てるのは最初の出力だけです**（E015）。二つ目からは、その出力と同じ名前の `define` から取ります — ここでは `define 付与点`。`result` を二本書くと E016 で止まります。

## 並びを順にたどって、一つの答えに畳む

呼び出し側が運賃行を何件か渡し、規則が上から順に見ていきます。一件ごとの判定は表がして、`fold` がその判定ごとに「次へ」「打ち切り」「これを採る」「持ち越す」を書き分けます。**件数の決まっていない入力を受けられる、ただ一つの形**です。

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

**この例が見せていること**

- **`elements` が一件ぶんの欄を宣言します。** 書き方は `inputs` と同じで、範囲も単位もそのまま効きます。呼び出し側は、この欄のそろった要素を何件でも渡します。
- **表の検査はいままでどおりです。** 一件の要素が一件の判定なので、完全性も重なりも単位も、これまでと同じように証明されます。上の表は `採用区分` の四つの値を過不足なく出します。
- **`fold` は、判定ごとの行き先を書くところです。** `next`（次へ）、`stop with <値>`（そこで打ち切る）、`take_unique <値>`（一件だけ採る。二件当たれば実行時にエラー）、`keep_max <値> by <鍵>`（鍵がいちばん大きいものを持ち越す）。行き先の無い判定があれば E024 で止まります。
- **要素ゼロ件のときと、最後まで見終えたときの答えは必須です**（E022・E023）。空の並びは必ず来ますし、「持ち越していたものを返す」も書いて初めて決まります（`exhausted -> held`）。
- **例は `sequence` に名前を付けて指します。** 例のセルに書けるのは値一つなので、並びのほうに名前を付けます。行がゼロ本の `sequence` が、要素ゼロ件の例です。
- **SQL と NumPy には生成しません。** 一つの問い合わせには、行から行へ値を持ち越して途中で打ち切る場所がないからです。NumPy のほうは、要素から要素へ状態を運ぶ走査が列の演算ではないからです。ほかの対象には出て、参照評価器と一致することを毎回確かめています。

## 並びを数えて、その数で判定する

請求書の宛名を、取引先台帳の候補と一件ずつ照合します。判定するのは表で、`count` がその判定に当てはまった件数を数え、**次の表がその数で手続きを決めます**。「一件なら自動、複数なら目視」を、人が数えてから渡すのではなく、規則の中で決められます。

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

**この例が見せていること**

- **`count` は歩いたあとに残る数です。** `count 一致数(hits) over 候補 where 照合結果 = 一致` は、要素ごとの判定が `一致` だった件数。そこから先は `number` の値なので、表の列に置けます。
- **数を判定に変えるのは、ふつうの表です。** `0` / `1` / `>=2` の境界に穴や重なりがあれば、いつもどおり検査が止めます。数えることと、数から決めることが、別々に検査に掛かります。
- **範囲は二つの意味を持ちます**（`range >=0 <=50`）。完全性の検査が見る全体集合であり、**並びの長さの上限**でもあります。51 件渡すと、生成コードが入口で断ります — 範囲外の数を断るのと同じことです。
- **数えるのは `count`、足すのは `sum` です**（下の「明細を足し上げる」）。平均は書けません——件数で割るのは変数で割ることなので、呼び出す手前で出します。
- **`fold` と `count` は一緒に書けません**（E031）。どちらも同じ並びの終わり方で、`fold` は途中で打ち切れるからです。

## 明細を足し上げる

「合計 5,000 円以上で送料無料」。何件あるか分からない明細を歩いて金額を足し、その合計で送料を決めます。`count` が件数を残すのと同じ場所に、`sum` は額を残します。

```rule
rule 買物かごの送料(cart_shipping) v1
description "明細の金額を合計して送料を決める。並びを合計する例"

enum 会員区分(tier) = 一般(regular) | 優待(premium)

inputs
  区分(tier) : 会員区分

# 一件ぶんの明細。呼び出し側は何件でも渡す
elements 明細(lines)
  金額(amount) : money[円]  range >=0円 <=100000円

# 歩いたあとに残るのは合計だけ。範囲は完全性の全体集合であり、走っている途中の合計が
# ここを出た時点で入口が断る境目でもある
sum 合計(total) over 明細 of 金額  range >=0円 <=1000000円

outputs
  送料(fee) : money[円]  round up(1円)

table 送料表(fee_table)
policy unique
| 合計             | 区分 | -> 送料(fee) : money[円] |
| >=5000円         | -    | 0円                      |
| >=3000円 <5000円 | 優待 | 0円                      |
| >=3000円 <5000円 | 一般 | 250円                    |
| <3000円          | -    | 500円                    |

examples
| 明細     | 区分 | -> 送料 |
| 二点     | 一般 | 0円     |
| 二点     | 優待 | 0円     |
| 少額     | 一般 | 500円   |
| 中くらい | 一般 | 250円   |
| 中くらい | 優待 | 0円     |
| 空       | 一般 | 500円   |

sequence 二点(two)
| 金額   |
| 3000円 |
| 2500円 |

sequence 少額(small)
| 金額   |
| 1200円 |

sequence 中くらい(middle)
| 金額   |
| 2000円 |
| 1500円 |

sequence 空(empty)
| 金額 |
```

**この例が見せていること**

- **`sum 合計(total) over 明細 of 金額` が残すのは一つの額です。** そこから先はふつうの列なので、`>=5000円` が軸の境目になり、完全性も重なりもいつもどおり止まります。
- **足す列は負になれません**（E029）。走っている途中の合計が上がる一方だから、宣言範囲を出た瞬間に入口が断れて、長さの分からない並びについても int64 の主張が本当になります。差を取りたいなら、非負の列を二つ足して引きます。
- **平均は書けません。** 件数で割るのは変数で割ることだからです（E115）。呼び出す手前で出して、値として渡します。

## 品番の頭で振り分ける

SKU の前置きで冷凍・冷蔵・常温を決めます。文字列の列に書けるのは前方一致だけで、それ以外は書けません。

```rule
rule 品番の扱い(sku_handling) v1
description "品番の前置きと箱数で、輸送の扱いを決める。文字列の前方一致の例"

enum 扱い(handling) = 冷凍便(frozen) | 冷蔵便(chilled) | 常温便(ambient) | 常温混載(ambient_mixed)

inputs
  品番(sku)  : string
  箱数(boxes) : number  range >=1 <=200

outputs
  扱い(handling) : 扱い

# 文字列は数え上げられないので、切れるのは前置きだけ。前置きの集合は有限なので、
# 完全性も重なりも、列挙の列とまったく同じように止まる
table 扱い表(handling_of)
policy first
| 品番                     | 箱数 | -> 扱い(handling) : 扱い |
| starts_with "FZ-"        | -    | 冷凍便                   |
| starts_with "CH-", "CL-" | -    | 冷蔵便                   |
| -                        | >=20 | 常温便                   |
| -                        | <20  | 常温混載                 |

examples
| 品番      | 箱数 | -> 扱い  |
| "FZ-1001" | 1    | 冷凍便   |
| "CH-2002" | 5    | 冷蔵便   |
| "CL-0003" | 5    | 冷蔵便   |
| "AB-0004" | 30   | 常温便   |
| "AB-0004" | 3    | 常温混載 |
```

**この例が見せていること**

- **前置きの集合は有限に切れます。** 文字列の全体は無限でも、その列のセルが名指す前置きから作られる分割は有限なので、§6.2 の仕組みがそのまま動きます — 穴があれば、それを起こす文字列がそのまま返ってきます。
- **比べるのはバイトです。** 大文字小文字も Unicode 正規化もしません。そうしないと 12 言語が同じ答えを返しません。
- **等号も集合も書けません**（E110）。値が数えられるなら `enum` のほうが向いています。部分一致と正規表現はありません。

## 比で配る按分

値引き総額を、明細に定価の比で割り付けます。上の「一件ずつに割る」が順に充てていく形なら、こちらは**比で配る**形です。一行ぶんは「ここまでの配分」から「直前までの配分」を引いた差になります。

```rule
rule 比例配分(pro_rata) v1
description "一括値引きを明細に定価の比で割り付ける。1 明細 = 1 回の判定で、累計は呼び出し側が持つ。端数は最後の明細に寄り、配った合計はかならず値引き総額に一致する"

inputs
  値引き総額(total_off)  : money[円]  range >=0円 <=100万円
  直前までの定価(before) : money[円]  range >=0円 <=1000万円
  ここまでの定価(upto)   : money[円]  range >=0円 <=1000万円
  定価合計(base)         : money[円]  range >=1円 <=1000万円
  対象(eligible)         : bool

constraint 直前までの定価 <= ここまでの定価
constraint ここまでの定価 <= 定価合計

outputs
  配分額(share) : money[円]  round down(1円)

derive 直前までの配分(to_before) : money[円] = allocate(値引き総額, 直前までの定価, 定価合計)  range >=0円 <=100万円
derive ここまでの配分(to_upto)   : money[円] = allocate(値引き総額, ここまでの定価, 定価合計)  range >=0円 <=100万円

define 差分(gap) : money[円] = ここまでの配分 - 直前までの配分

table 配分可否(applies)
policy unique
| 対象  | -> 配る(on) : money[円] |
| true  | 差分                    |
| false | 0円                     |

result 配分額 = 配る

examples
| 値引き総額 | 直前までの定価 | ここまでの定価 | 定価合計 | 対象  | -> 配分額 |
| 1000円     | 0円            | 3000円         | 10000円  | true  | 300円     |
| 1000円     | 3000円         | 10000円        | 10000円  | true  | 700円     |
| 100円      | 0円            | 333円          | 1000円   | true  | 33円      |
| 100円      | 333円          | 666円          | 1000円   | true  | 33円      |
| 100円      | 666円          | 1000円         | 1000円   | true  | 34円      |
| 0円        | 0円            | 500円          | 1000円   | true  | 0円       |
| 1000円     | 0円            | 3000円         | 10000円  | false | 0円       |
```

**この例が見せていること**

- **`allocate(配る額, 累計, 全体)` は、定数でないもので割れる唯一の場所です。** 答えは `配る額 × 累計 ÷ 全体` を下に丸めた値で、単位の整数になります。
- **差にすると、配った合計が総額にぴったり一致します。** 隣り合う行で同じ値が足されて引かれるので、端数は最後の行に寄ります。`proofs/` に定理があります（`runTotal_exact`）。
- **`constraint 累計 <= 全体` が要ります**（E117）。これが無いと配る分が配る額を超えることがあり、区間も二つの範囲の積になってしまいます。制約は二行に分けて書いても、つながって効きます。

## 呼び出し側の注文から入力を取る（JSON Schema）

注文オブジェクトから送料を決めます。呼び出し側はもう JSON Schema で注文の形を決めているので、それを `shape` で借り、入力ごとにその中のどこから来るかを `from` で書きます。表そのものは、ほかの例と同じ平たい入力の表です。

```rule
rule 注文の送料(order_shipping) v1
description "注文オブジェクトから送料を決める。呼び出し側の契約から入力を取り出す例（§15.125）"

# 呼び出し側はもう JSON Schema で注文の形を決めている。それを借りて、入力がその形の
# どこから来るかを書くと、平たくするコードは生成され、パスは毎回の check で契約に照らされる。
shape 注文(order) = jsonschema "contracts/order.schema.json" "#/$defs/Order"

enum 地域(zone) = honshu(honshu) | hokkaido(hokkaido) | okinawa(okinawa)

inputs
  あて先(zone)   : 地域    from 注文.shipping.zone
  冷蔵あり(cold) : bool    from any 注文.lines where chilled = true
  明細数(lines)  : number  range >=1 <=50  from count 注文.lines

outputs
  送料(fee) : money[円]  round up(10円)

# 冷蔵は地域を問わず 500円 増し、明細が 10 件を超えると 200円 増し。
table 地域別(by_zone)
policy unique
| あて先   | -> 地域料(zone_fee) : money[円] |
| honshu   | 800円                           |
| hokkaido | 1200円                          |
| okinawa  | 1500円                          |

table 冷蔵加算(cold_extra)
policy unique
| 冷蔵あり | -> 冷蔵料(cold_fee) : money[円] |
| true     | 500円                           |
| false    | 0円                             |

table 件数加算(bulk_extra)
policy unique
| 明細数 | -> 件数料(bulk_fee) : money[円] |
| >10    | 200円                           |
| <=10   | 0円                             |

define 合計(total) : money[円] = 地域料 + 冷蔵料 + 件数料

result 送料 = 合計

examples
| あて先   | 冷蔵あり | 明細数 | -> 送料 |
| honshu   | false    | 1      | 800円   |
| honshu   | true     | 1      | 1300円  |
| okinawa  | true     | 11     | 2200円  |
| hokkaido | false    | 10     | 1200円  |
```

規則が読む契約（`contracts/order.schema.json`）:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Order",
  "$defs": {
    "Order": {
      "type": "object",
      "properties": {
        "id": { "type": "string" },
        "shipping": { "$ref": "#/$defs/Shipping" },
        "lines": { "type": "array", "minItems": 1, "maxItems": 50, "items": { "$ref": "#/$defs/Line" } }
      },
      "required": ["shipping", "lines"]
    },
    "Shipping": {
      "type": "object",
      "properties": {
        "zone": { "type": "string", "enum": ["honshu", "hokkaido", "okinawa"] },
        "postcode": { "type": "string" }
      },
      "required": ["zone"]
    },
    "Line": {
      "type": "object",
      "properties": {
        "sku": { "type": "string" },
        "chilled": { "type": "boolean" },
        "amount_jpy": { "type": "integer" }
      },
      "required": ["sku", "chilled", "amount_jpy"]
    }
  }
}
```

**この例が見せていること**

- **`from` の形は四つです。** フィールドの値（`from 注文.shipping.zone`）、要素のどれかが当てはまるか（`from any 注文.lines where chilled = true`）、全部が当てはまるか（`from all …`）、何件あるか（`from count 注文.lines`）。結合や入れ子の量化は書けません。
- **`rulec gen` は `order_shipping_from(order)` も書きます。** 注文オブジェクトをそのまま渡すと、入力を取り出して規則を呼びます。書くのは、呼び出し側がオブジェクトをただの連想配列として持っている五つの言語です。
- **パスは `rulec check` のたびに契約に照らされます。** 契約に無いフィールドを名指しすれば E121 で、どこまで届いたかと、そこにあったフィールドを言います。型が合わなければ E120 です。
- **契約の検証も、入力の宣言と突き合わせます。** 契約は `lines` を 1〜50 件（`minItems`・`maxItems`）に限っていて、規則の `明細数` の `range >=1 <=50` と同じです。`maxItems` を消すと、51 件の注文は契約を通るのに規則は断るので、E122 で止まります。`fix.text` は契約に書き足すキーワード（`"minItems": 1, "maxItems": 50`）そのものです。
- **表の検査には触れません。** 射影から出てくるのはただのスカラーの入力で、完全性も重なりも、`from` が無いときと同じに決まります。

## Connect の要求から入力を取る（.proto と Protovalidate）

出荷の要求から送料を決めます。要求の形は `.proto` で決まっていて、フィールドには Protovalidate の注釈が付いています。規則はその `.proto` を借り、入力の宣言を注釈とそろえてあります。

```rule
rule 出荷の送料(shipment_fee) v1
description "出荷の要求（.proto）から送料を決める。Protovalidate の注釈と入力の宣言をそろえた例（§15.132、§15.133）"

# 呼び出し側は Connect のサービスで、要求の形は .proto で決まっている。それを借りて、入力が
# 要求のどこから来るかを書く。生成する shipment_fee_from は、protojson で届いた要求をそのまま読む。
shape 出荷(shipment) = proto "contracts/shipment.proto" shop.v1.CreateShipmentRequest

enum 地域(region) = honshu(honshu) | hokkaido(hokkaido) | okinawa(okinawa)
enum 時間帯(window) = morning(morning) | evening(evening)

inputs
  あて先(region)      : 地域  from 出荷.destination.region
  割れ物あり(fragile) : bool  from any 出荷.parcels where handling = HANDLING_FRAGILE
  個数(parcels)       : number  range >=1 <=20  from count 出荷.parcels
  申告額(declared)    : money[円]  range >=0円 <=100万円  from 出荷.declared_value_jpy
  時間帯(window)      : 時間帯?  from 出荷.delivery_window

outputs
  送料(fee) : money[円]  round up(10円)

# 一個あたりの運賃に個数を掛け、割れ物・補償・時間帯指定の加算を足す。
table 地域別(by_region)
policy unique
| あて先   | -> 一個の運賃(per_parcel) : money[円] |
| honshu   | 800円                                 |
| hokkaido | 1200円                                |
| okinawa  | 1500円                                |

table 割れ物加算(fragile_extra)
policy unique
| 割れ物あり | -> 割れ物料(fragile_fee) : money[円] |
| true       | 300円                                |
| false      | 0円                                  |

table 補償(insurance)
policy unique
| 申告額          | -> 補償料(insurance_fee) : money[円] |
| <=3万円         | 0円                                  |
| >3万円 <=10万円 | 200円                                |
| >10万円         | 500円                                |

table 時間帯指定(window_extra)
policy unique
| 時間帯           | -> 指定料(window_fee) : money[円] |
| none             | 0円                               |
| morning, evening | 100円                             |

define 合計(total) : money[円] = 一個の運賃 × 個数 + 割れ物料 + 補償料 + 指定料

result 送料 = 合計

examples
| あて先   | 割れ物あり | 個数 | 申告額  | 時間帯  | -> 送料 |
| honshu   | false      | 1    | 0円     | none    | 800円   |
| hokkaido | true       | 2    | 5万円   | none    | 2900円  |
| okinawa  | false      | 3    | 20万円  | evening | 5100円  |
| honshu   | true       | 20   | 100万円 | morning | 16900円 |
```

規則が読む契約（`contracts/shipment.proto`）:

```proto
syntax = "proto3";

package shop.v1;

import "buf/validate/validate.proto";

// The request the shipping service takes, validated by Protovalidate before anything reads it.
message CreateShipmentRequest {
  Destination destination = 1 [(buf.validate.field).required = true];
  repeated Parcel parcels = 2 [(buf.validate.field).repeated = {min_items: 1, max_items: 20}];
  int64 declared_value_jpy = 3 [(buf.validate.field).int64 = {gte: 0, lte: 1000000}];
  optional string delivery_window = 4 [(buf.validate.field).string = {in: ["morning", "evening"]}];
}

message Destination {
  string region = 1 [(buf.validate.field).string = {in: ["honshu", "hokkaido", "okinawa"]}];
  string postcode = 2;
}

message Parcel {
  int64 weight_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];
  Handling handling = 2;
}

enum Handling {
  HANDLING_STANDARD = 0;
  HANDLING_FRAGILE = 1;
}
```

**この例が見せていること**

- **生成する `shipment_fee_from` は、protojson の JSON をそのまま読みます。** フィールドは lowerCamelCase の名前（`declaredValueJpy`）でも `.proto` に書いた名前（`declared_value_jpy`）でも読み、省かれたフィールドは proto の既定値として読みます。int64 は文字列で届いても、数として読みます。
- **`optional` のフィールドは `T?` の入力で受けます。** `delivery_window` は送られないことがあるので `時間帯?` にして、無いとき（`none`）の行を表に書いています。
- **列挙のフィールドは、`where` で値の名前を比べます。** `handling` は `.proto` の列挙で、protojson は値の名前（`HANDLING_FRAGILE`）を運びます。番号 0 の値（`HANDLING_STANDARD`）のときはフィールドごと省かれますが、そのときも `HANDLING_STANDARD` として読みます。
- **注釈と宣言がそろっているので、`check` は通ります。** `destination` から `required = true` を外すと、要求は `destination` を省けるようになり、そのとき `region` は `""` として届きます。`地域` は `""` を受け付けないので E122 で止まり、`fix.text` は `[(buf.validate.field).required = true]` です。

## 契約がフィールドのあいだに置く条件（CEL）

見積の要求から運賃を決めます。要求を検証する `.proto` は、Protovalidate の CEL で二つのことを約束しています。速達は 5kg まで、申告額は補償額を超えない、の二つです。規則はその約束に乗って書いてあります。

```rule
rule 速達の見積(express_quote) v1
description "見積の要求（.proto）から運賃を決める。契約がフィールドのあいだに置く条件を、規則の制約と行に照らす例（§15.140）"

# 呼び出し側の契約は、速達を 5kg までに限り、申告額が補償額を超えないことを CEL で約束している。
# 規則はその約束を constraint に書き、補償料の表はそれに頼って、申告額が補償額を超える行を持たない。
shape 見積(quote) = proto "contracts/quote.proto" shop.v1.QuoteRequest

inputs
  重さ(weight)     : mass[g]    range >=1g <=30kg      from 見積.weight_g
  速達(express)    : bool                              from 見積.express
  申告額(declared) : money[円]  range >=0円 <=30万円  from 見積.declared_jpy
  補償額(cover)    : money[円]  range >=0円 <=30万円  from 見積.cover_jpy

constraint 申告額 <= 補償額

outputs
  運賃(fee) : money[円]  round up(10円)

# 速達は 2kg を境に二段。契約が 5kg を超える速達を通さないので、その先の行は書かない。
table 重さ別(by_weight)
policy unique
| 速達  | 重さ        | -> 基本料(base) : money[円] |
| false | <=2kg       | 700円                       |
| false | >2kg <=10kg | 1100円                      |
| false | >10kg       | 1600円                      |
| true  | <=2kg       | 1200円                      |
| true  | >2kg        | 1800円                      |

table 補償(insurance)
policy unique
| 申告額  | 補償額  | -> 補償料(cover_charge) : money[円] |
| <=5万円 | <=5万円 | 0円                                 |
| <=5万円 | >5万円  | 300円                               |
| >5万円  | >5万円  | 600円                               |

define 合計(total) : money[円] = 基本料 + 補償料

result 運賃 = 合計

examples
| 重さ | 速達  | 申告額 | 補償額 | -> 運賃 |
| 1kg  | false | 0円    | 0円    | 700円   |
| 3kg  | true  | 2万円  | 10万円 | 2100円  |
| 12kg | false | 8万円  | 10万円 | 2200円  |
| 2kg  | true  | 5万円  | 5万円  | 1200円  |
```

規則が読む契約（`contracts/quote.proto`）:

```proto
syntax = "proto3";

package shop.v1;

import "buf/validate/validate.proto";

// A quote for one parcel, validated by Protovalidate before anything reads it. Two of its
// rules relate one field to another: express takes a parcel of up to 5 kg, and the value
// declared never exceeds the cover the caller chose.
message QuoteRequest {
  option (buf.validate.message).cel = {
    id: "express_weight",
    message: "express takes a parcel of up to 5 kg",
    expression: "!this.express || this.weight_g <= 5000"
  };
  option (buf.validate.message).cel = {
    id: "declared_within_cover",
    message: "the declared value exceeds the cover",
    expression: "this.declared_jpy <= this.cover_jpy"
  };

  int64 weight_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];
  bool express = 2;
  int64 declared_jpy = 3 [(buf.validate.field).int64 = {gte: 0, lte: 300000}];
  int64 cover_jpy = 4 [(buf.validate.field).int64 = {gte: 0, lte: 300000}];
}
```

**この例が見せていること**

- **`constraint 申告額 <= 補償額` は、契約が約束しているから書けます。** 補償料の表には、申告額が補償額を超える行がありません。制約があるので、完全性の検査はその組み合わせに行を求めません。契約の CEL（`this.declared_jpy <= this.cover_jpy`）が同じことを約束しているので、`check` は通ります。制約を `<` にすると、契約は申告額と補償額が等しい要求を通すので E123 で止まり、その要求を例に出します。
- **5kg を超える速達の行は書いていません。** 契約の `!this.express || this.weight_g <= 5000` が、その要求を通さないからです。書き足すと、その行は W124 になります。セルを一つずつ見れば契約の通す値なのに、組み合わせとしては通らない行だからです。
- **CEL は、読める部分を読みます。** 整数の一次式の比較、`in`、`size()`、`has()`、`&&`・`||`・`!`・`? :` です。剰余や文字列の関数のように読めない部分は真として扱うので、見逃すことはありません。

## 返品できるかどうかを英語で書く

金額がどこにも出てこない例を、名前もセルも英語で書いたものです。答えは四つの語のどれか一つで、入力の組み合わせはどれもちょうど一行に当たります。お店の規約を想定した作り物で、どこかの規約の転記ではありません。

```rule
rule return_eligibility v1
description "Whether a return is accepted. Written in English, and with no money in it anywhere: the answer is a class"

# A sketch of a shop's own terms, not a transcription. The shape is the point: the answer is
# one of four words, and every combination of the four inputs reaches exactly one row.

enum category = electronics | clothing | perishable
enum verdict = accepted | outside_window | condition_failed | not_returnable

inputs
  item    : category
  days    : number  range >=0 <=365
  opened  : bool
  receipt : bool

outputs
  answer : verdict

# Written as `policy unique`, so the rows are disjoint and the checker proves that every
# input reaches exactly one of them. `policy first` would take three rows fewer by letting
# the refusals at the top swallow the rest — and then which row answers a given case would
# be a question about the order of the rows rather than about the row itself.
table decide
policy unique
| item            | receipt | days | opened | -> answer : verdict |
| perishable      | -       | -    | -      | not_returnable      |
| not: perishable | false   | -    | -      | condition_failed    |
| electronics     | true    | >14  | -      | outside_window      |
| electronics     | true    | <=14 | true   | condition_failed    |
| electronics     | true    | <=14 | false  | accepted            |
| clothing        | true    | >30  | -      | outside_window      |
| clothing        | true    | <=30 | -      | accepted            |

examples
| item        | days | opened | receipt | -> answer        |
| clothing    | 10   | true   | true    | accepted         |
| clothing    | 31   | false  | true    | outside_window   |
| electronics | 3    | true   | true    | condition_failed |
| electronics | 14   | false  | true    | accepted         |
| perishable  | 0    | false  | true    | not_returnable   |
| clothing    | 10   | false  | false   | condition_failed |
```

**この例が見せていること**

- **答えが金額でない規則も、形は同じです。** 出力は列挙の値で、丸めの宣言は要りません。
- **`policy unique` で書いてあります。** 行が重ならないように書くと、どの入力もちょうど一行に当たることを検査が証明します。`policy first` なら三行減らせますが、そのぶん「どの行が答えたのか」が行の並び順の話になります。
- **`not:` は列挙の値そのものにも使えます。** `not: perishable` は「生鮮以外」で、グループを作らなくても書けます。

## ポンドとインチの運賃表

英語で書いた運賃表です。重さはポンド、寸法はインチ、金額は USD。これも作り物で、金額は架空のものです。コーパスでヤード・ポンド法の単位を使っているのはこの規則だけです。

```rule
rule parcel_rate v1
description "A parcel tariff in pounds and inches, written in English. A sketch, not a transcription: the amounts are made up"

# Nothing else in the corpus is priced in USD by weight, and nothing at all reached oz, lb
# or in — an unexercised unit is an unchecked unit (§15.9).

enum size_class = envelope | small | large
enum zone = domestic | canada | overseas

group north_america = domestic, canada

inputs
  weight    : mass[lb]    range >=1lb <=70lb
  girth     : length[in]  range >=1in <=130in
  dest      : zone
  signature : bool

outputs
  fee : money[USD, incl_tax]  round up(1USD)

# One table decides the class and the next one prices it: what the first produces is a
# column of the second.
table size_of
policy first
| girth  | -> size : size_class |
| <=22in | envelope             |
| <=60in | small                |
| -      | large                |

table base_rate
policy unique
| dest          | size     | weight  | -> base : money[USD, incl_tax] |
| north_america | envelope | -       | 6USD                           |
| north_america | small    | <=160oz | 12USD                          |
| north_america | small    | >160oz  | 18USD                          |
| north_america | large    | <=160oz | 22USD                          |
| north_america | large    | >160oz  | 30USD                          |
| overseas      | envelope | -       | 16USD                          |
| overseas      | small    | -       | 38USD                          |
| overseas      | large    | -       | 60USD                          |

# A fuel surcharge is a percentage of the base, which is what the rounding on the output is
# there to settle: 12USD at 5% is 12.60USD, and up(1USD) makes that 13USD.
table fuel_rate
policy unique
| dest          | -> fuel : rate[step 1%] |
| north_america | 5%                      |
| overseas      | 12%                     |

table signature_fee
policy unique
| signature | -> extra : money[USD, incl_tax] |
| true      | 4USD                            |
| false     | 0USD                            |

result fee = base + base × fuel + extra

examples
| weight | girth | dest     | signature | -> fee |
| 5lb    | 10in  | domestic | false     | 7USD   |
| 5lb    | 40in  | canada   | false     | 13USD  |
| 20lb   | 40in  | domestic | true      | 23USD  |
| 5lb    | 10in  | overseas | false     | 18USD  |
```

**この例が見せていること**

- **ヤード・ポンド法の単位も、ほかの単位と同じに扱えます。** `mass[lb]` の入力を、行では `160oz` と書いて引いています（16oz = 1lb）。単位は型の一部なので、`in` の列に `cm` と書けば E103 で止まります。
- **表を積んでいます。** 上の表が出す `size` が、下の表の列になります。寸法から区分を決める表と、区分から値段を決める表を分けて書けるということです。
- **丸めの宣言が効くのはこういうところです。** 基本料に 5% を掛けると 12.60USD のような端数が出ます。`up(1USD)` と書いてあるので 13USD になりますが、どちらに寄せるかは商売の決めごとで、道具の側では決めません。

## EU 旅客権利規則を英語で書く

名前もセルも英語なので、ASCII 別名が一つも出てきません。金額は EUR、距離は km です。公開されている法令（(EC) No 261/2004 第 7 条）をそのまま写したもので、条文そのものが決定表の形をしています。

```rule
rule ec261 v1
description "Article 7 of EU air passenger rights Regulation (EC) No 261/2004. The example in English, in EUR and km"

# Published law, transcribed as it stands: the amounts are paragraph 1, the halving is
# paragraph 2. Distance and whether the flight is intra-EU decide the band, and that one
# band decides both the amount and the time threshold.

enum band = short | medium | long

inputs
  distance : length[km]  range >=1km <=20000km
  intra_eu : bool
  delay    : duration[h]  range >=0h <=48h

outputs
  compensation : money[EUR, incl_tax]  round down(1EUR)

# Article 7(1). (a) 1500km or less, (b) intra-EU over 1500km and other flights of 1500 to
# 3500km, (c) everything else. Whether "between 1500 and 3500 kilometres" takes in either
# end cannot be read out of the text. Since (a) is "1500 kilometres or less", the lower end
# is open here — and having to decide it is the point of the tool: undecided, there is no
# table to write.
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

# Article 7(2). The amount may be halved where the re-routing arrives no later than
# (a) two hours, (b) three hours, (c) four hours after the scheduled time. The article's
# (a)(b)(c) restate the distance conditions of paragraph 1 in full, so this table keys on
# distance too, not on the band. The band would fit in two rows, and would put "which
# distance gets four hours" one step further from the text.
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

**この例が見せていること**

- **ASCII の名前には別名が要りません。** 漢字は Go の公開識別子になれないので `運賃(fee)` のような別名が要りますが、`distance` にはその必要がありません。英語で書けば、カッコはどこにも出てきません。
- **通貨どうしは換算されません。** `money[EUR]` の列に `100円` を書くと E103 で止まります。為替レートはこの道具の中に無く、あってはいけないものだからです（[単位の一覧](reference.md)）。
- **条文が決めていないことを、表が決めさせます。** 第 7 条 1 項の (b) は「between 1500 and 3500 kilometres」で、両端を含むかが読めません。(a) が「1500km 以下」なので下端は開く、とここで決めています。決めなければ抜けか重なりで止まるので、**あいまいなまま先へは進めません**。
- **同じ条件を二度書くほうが正しいこともあります。** 5 割引きの閾値（2 / 3 / 4 時間）は帯で引けば二列で済みますが、条文の 2 項は距離の条件を丸ごと書き直しています。ここでも距離で引いたのは、そのほうが原文と行が一対一で並ぶからです。

## 最低賃金と、それを上書きする例外

2026 年 4 月からの英国の最低賃金です。GOV.UK は年齢帯ごとの時給を並べ、そのあとに「見習いはこの額」という例外を書いています。だから表も二つで、二つめが一つめを上書きします。

```rule
rule uk_minimum_wage v1
description "The UK hourly minimum wage from 1 April 2026, transcribed from GOV.UK. An English transcription with a main rule and its exception"

# GOV.UK states a rate for each age band, and then states the apprentice rate as an
# exception to it: an apprentice takes the apprentice rate while under 19, or while in the
# first year of the apprenticeship, and the rate for their age afterwards. Two tables, the
# second overriding the first, is that sentence. One table with an apprentice column would
# give the same answers with the shape of the source lost.
source gov = file "sources/uk-nmw.md" sha256:bc45eedf7f908896  # GOV.UK, Open Government Licence v3.0
  table1 sha256:3c2fa10da7e3bf65

inputs
  age        : number  range >=16 <=70
  apprentice : bool
  first_year : bool

# A wage is not a price, so there is no tax flag on it. The rates are whole pence, and the
# rounding is declared because every numeric output must declare one.
outputs
  hourly : money[GBPc]  round down(1GBPc)

table by_age  @gov table1
policy unique
| age       | -> hourly : money[GBPc] |
| <18       | 800GBPc                 |
| >=18 <=20 | 1085GBPc                |
| >=21      | 1271GBPc                |

table apprentice_rate  @gov table1
policy unique
overrides by_age
| apprentice | first_year | age  | -> hourly : money[GBPc] |
| true       | -          | <19  | 800GBPc                 |
| true       | true       | >=19 | 800GBPc                 |

examples
| age | apprentice | first_year | -> hourly |
| 25  | false      | false      | 1271GBPc  |
| 19  | false      | false      | 1085GBPc  |
| 17  | false      | false      | 800GBPc   |
| 25  | true       | true       | 800GBPc   |
| 25  | true       | false      | 1271GBPc  |
| 18  | true       | false      | 800GBPc   |
```

**この例が見せていること**

- **本則と例外は、一枚の広い表ではなく二枚の表です。** `overrides by_age` が「見習いの行が優先する」と宣言し、検査は二つをひとつの集合として完全性と重なりに掛けます。例外が本則に穴を空けることはできません。
- **文書は規則の隣に置いて、digest で留めてあります。** `source gov = file "…" sha256:…` と、表に付けた `@gov table1` です。ここから先は、写しに無い時給を書くと E116 で止まります。
- **見習いの一文が二行になっているのは、条件が二つだからです。** 「19 歳未満」と「19 歳以上で見習い一年目」を、ページが書いているとおりに二行で書いています。

## 逓減する控除と、その上の税率帯

英国の個人控除（Personal Allowance）と、所得がどの税率帯に入るかです。控除は 10 万ポンドを超えた分 2 ポンドにつき 1 ポンドずつ減ります。これは行ではなく計算なので、`derive` に書いて、それを行が名前で呼びます。

```rule
rule uk_income_tax v1
description "The UK Personal Allowance and the Income Tax band an income falls in, for England, Wales and Northern Ireland. Transcribed from GOV.UK"

source gov = file "sources/uk-income-tax.md" sha256:8aa392743f75ce6f  # GOV.UK, Open Government Licence v3.0
  table1 sha256:7d9ba57e7bb838a7

enum band = personal_allowance | basic | higher | additional

inputs
  income : money[GBP]  range >=0GBP <=10000000GBP

outputs
  allowance : money[GBP]        round down(1GBP)
  in_band   : band
  rate      : rate[step 1%]  round down(1%)

# "Your personal allowance goes down by £1 for every £2 that your adjusted net income is
# above £100,000." Half of the excess, taken off the standard allowance — and because it is
# £1 per £2, an odd pound is dropped, which is what the rounding on the output settles.
derive above_100k : money[GBP] = income − 100000GBP           range >=-100000GBP <=9900000GBP
derive tapered    : money[GBP] = 12570GBP − above_100k × 50%  range >=-4937430GBP <=62570GBP

# The third row could be left to the taper, which reaches zero at £125,140 on its own. It is
# written out because the page writes it out: "your allowance is zero if your income is
# £125,140 or above".
table allowance_of
policy unique
| income                | -> allowance : money[GBP] |
| <=100000GBP           | 12570GBP                  |
| >100000GBP <125140GBP | tapered                   |
| >=125140GBP           | 0GBP                      |

# The page writes each band from the first pound that falls in it (£12,571 to £50,270). A
# pound is the unit here, so "from £12,571" and "above £12,570" are the same set, and the
# second form is the one the checker reads as adjacent to the row above.
table band_of  @gov table1
policy unique
| income                | -> in_band : band  | rate : rate[step 1%] |
| <=12570GBP            | personal_allowance | 0%                   |
| >12570GBP <=50270GBP  | basic              | 20%                  |
| >50270GBP <=125140GBP | higher             | 40%                  |
| >125140GBP            | additional         | 45%                  |

examples
| income    | -> allowance | in_band            | rate |
| 10000GBP  | 12570GBP     | personal_allowance | 0%   |
| 30000GBP  | 12570GBP     | basic              | 20%  |
| 100000GBP | 12570GBP     | higher             | 40%  |
| 110000GBP | 7570GBP      | higher             | 40%  |
| 125140GBP | 0GBP         | higher             | 40%  |
| 200000GBP | 0GBP         | additional         | 45%  |
```

**この例が見せていること**

- **行には計算した値の名前を書けます。** `allowance_of` の真ん中の行が `tapered` という `derive` を指しています。上下の行は、ページが書いている二つの金額そのものです。
- **「2 ポンドにつき 1 ポンド」は率の掛け算で、端数は丸めの宣言が決めます。** 出力に書いた `round down(1GBP)` がそれで、宣言は省けません。
- **ページの「£12,571 から」は、ここでは「£12,570 を超える」と書いてあります。** 単位がポンドなので同じ集合ですが、後者のほうが上の行と隣り合っていることを検査が読み取れます。

## 米国の連邦所得税を、段ごとに

2025 年分、単身者の連邦所得税です。IRS の税率表（Rev. Proc. 2024-40）からの転記で、日本の速算表と形は同じ — 段と、税率と、そこから足し始める金額。

```rule
rule us_income_tax v1
description "The 2025 federal income tax of an unmarried individual, transcribed from the IRS rate table. The English counterpart of 所得税.rule"

source irs = file "sources/us-tax-rate-tables.md" sha256:0abe29cfe35eeb22  # Rev. Proc. 2024-40, a US government work
  table1 sha256:94648a4eb1ff7b80

inputs
  taxable : money[USDc]  range >=0USDc <=100000000000USDc  # up to a billion dollars, in cents

outputs
  tax : money[USDc]  round down(1USDc)

# The table states each bracket as "$X plus Y% of the excess over $Z", so all three of X, Y
# and Z are transcribed and the arithmetic is written out below. Folding them into one
# deduction — the form Japan's own quick table uses — would be a number the source does not
# print, and E116 would be right to stop it.
#
# The citation sits on the rows rather than on the table: the first bracket is stated as
# "10% of the taxable income", with no amount in it, so its two zeros are this rule's way of
# writing that and not something the copy shows.
table brackets
policy unique
| taxable                      | -> base : money[USDc] | rate : rate[step 1%] | floor : money[USDc] |
| <=1192500USDc                | 0USDc                 | 10%                  | 0USDc               |
| >1192500USDc <=4847500USDc   | 119250USDc            | 12%                  | 1192500USDc         |  @irs table1
| >4847500USDc <=10335000USDc  | 557850USDc            | 22%                  | 4847500USDc         |  @irs table1
| >10335000USDc <=19730000USDc | 1765100USDc           | 24%                  | 10335000USDc        |  @irs table1
| >19730000USDc <=25052500USDc | 4019900USDc           | 32%                  | 19730000USDc        |  @irs table1
| >25052500USDc <=62635000USDc | 5723100USDc           | 35%                  | 25052500USDc        |  @irs table1
| >62635000USDc                | 18876975USDc          | 37%                  | 62635000USDc        |  @irs table1

define excess : money[USDc] = taxable − floor
define tax    : money[USDc] = base + excess × rate

examples
| taxable       | -> tax       |
| 1000000USDc   | 100000USDc   |  # $10,000, all of it in the first bracket
| 1192500USDc   | 119250USDc   |  # the top of the first bracket, which is the second one's base
| 5000000USDc   | 591400USDc   |  # $50,000: $5,578.50 + 22% of $1,525
| 100000000USDc | 32702025USDc |  # $1,000,000: $188,769.75 + 37% of $373,650
```

**この例が見せていること**

- **「$X plus Y% of the excess over $Z」は、三つの列と二行の計算になります。** base・rate・floor をそのまま写し、`excess` と `tax` を `define` で書いています。控除額ひとつに畳むと、出典が印刷していない数が出てくるので、そうしていません。
- **最初の段にだけ出典が付いていません。** ページは「10% of the taxable income」と書くだけで金額を書いていないので、その行の二つのゼロは規則側の書き方です。出典は表ではなく行に付けてあり、行の出典は「その行がどこから来たか」だけを言います。
- **ドルではなくセントです。** $1,192.50 は整数のドルではないので `money[USDc]` で数えます。単位は型の一部なので、ドルとセントが取り違えられることもありません。

## 米国の連邦規則から、条ひとつ

消火器まで何フィート歩くことになるか。29 CFR 1910.157(d) の転記で、条文は eCFR から日付を指定して取ってきた写しに留めてあります。日本の法令を e-Gov に留めるのと同じ仕組みが、そのまま英語圏の法令で動きます。

```rule
rule osha_extinguisher v1
description "How far an employee may have to walk to a portable fire extinguisher. Transcribed from 29 CFR 1910.157(d), read out of the eCFR"

# The rule an English-speaking reader gets from a statute database, as the Japanese rules get
# theirs from e-Gov: the section is fetched as of a date, kept as a copy beside the rule and
# pinned, and `rulec source outdated` asks the eCFR whether a later amendment touched it.
source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269

enum fire_class = a | b | c | d
enum pattern = class_a | class_b

# (d)(5) sends a Class C hazard to "the appropriate pattern for the existing Class A or Class
# B hazards", so which of the two is present has to be an input. For the other three classes
# it is not read, and the `-` cells below say so.
inputs
  hazard : fire_class
  nearby : pattern

outputs
  travel : length[ft]  round down(1ft)

# The section states the distances in feet with the metre in brackets — "75 feet (22.9 m)" —
# so feet is the unit the rule is written in. A length is one integer in its declared unit,
# and there is no conversion to decide.
table distance  @osha "§1910.157"
policy unique
| hazard | nearby  | -> travel : length[ft] |
| a      | -       | 75ft                   |
| b      | -       | 50ft                   |
| c      | class_a | 75ft                   |
| c      | class_b | 50ft                   |
| d      | -       | 75ft                   |

examples
| hazard | nearby  | -> travel |
| a      | class_a | 75ft      |
| b      | class_a | 50ft      |
| c      | class_b | 50ft      |
| d      | class_a | 75ft      |
```

**この例が見せていること**

- **法令データベースは `law` の後の語で選びます。** `law ecfr "29 CFR 1910"` の `ecfr` がそれで、省略すると e-Gov です。id は title と part、引くのは section ひとつです。
- **語として読めない箇所は引用符で囲みます。** `@osha "§1910.157"` のように。写しは `sources/law/29-CFR-1910@2026-01-01/1910.157.xml` に置かれ、`rulec source pin` がそのハッシュを書きます。
- **`rulec source outdated` が改正を教えます。** eCFR はその section の改正日を返し、体裁だけの直しかどうかも言うので、本文が動いたときだけ読み直しになります。
- **単位はフィートです。** 条文が「75 feet (22.9 m)」と書くので `length[ft]` で写しました。換算はしません。

## 英国の印紙税、本則と軽減

住宅の売買にかかる SDLT が、どの税率帯に入るか。GOV.UK からの転記で、**本則の表と、初めて家を買う人の軽減の表**という形が `印紙税の本則と軽減.rule` とそのまま重なります。

```rule
rule uk_stamp_duty v1
description "The SDLT rate band a residential purchase falls in, and the surcharge on a second home. Transcribed from GOV.UK. The English counterpart of 印紙税の本則と軽減.rule"

source gov = file "sources/uk-sdlt.md" sha256:799106ea8a821a00  # GOV.UK, Open Government Licence v3.0
  table1 sha256:360409a5675d3552
  table2 sha256:15d4ed0baca64189

inputs
  price      : money[GBP]  range >=0GBP <=20000000GBP
  first_time : bool
  additional : bool

outputs
  band      : rate[step 1%]  round down(1%)
  surcharge : rate[step 1%]  round down(1%)

# The main rule. The first row carries no citation: the page writes that band as "Zero"
# rather than as a percentage, so `0%` is this rule's way of writing it.
table standard
policy unique
| price                   | -> band : rate[step 1%] |
| <=125000GBP             | 0%                      |
| >125000GBP <=250000GBP  | 2%                      |  @gov table1
| >250000GBP <=925000GBP  | 5%                      |  @gov table1
| >925000GBP <=1500000GBP | 10%                     |  @gov table1
| >1500000GBP             | 12%                     |  @gov table1

# The relief, which stops at £500,000: "If the price is over £500,000, you cannot claim".
# Above that the main rule shows through on its own, which is what `overrides` on a table
# that covers only part of the input space means. A first-time buyer who already owns a
# property is not one, so the rows say so rather than leaving it to the reader.
table first_time_relief  @gov table2
policy unique
overrides standard
| first_time | additional | price                  | -> band : rate[step 1%] |
| true       | false      | <=300000GBP            | 0%                      |
| true       | false      | >300000GBP <=500000GBP | 5%                      |

# "You'll usually have to pay 5% on top of SDLT rates if buying a new residential property
# means you'll own more than one" is a sentence, not a table, so this cites the document
# whole. What it is on top of is the band above; the two are not added here, because the tax
# itself is worked out slice by slice and the page tabulates no such total.
table second_home  @gov
policy unique
| additional | -> surcharge : rate[step 1%] |
| true       | 5%                           |
| false      | 0%                           |

examples
| price      | first_time | additional | -> band | surcharge |
| 100000GBP  | false      | false      | 0%      | 0%        |
| 200000GBP  | false      | false      | 2%      | 0%        |
| 200000GBP  | true       | false      | 0%      | 0%        |
| 400000GBP  | true       | false      | 5%      | 0%        |
| 600000GBP  | true       | false      | 5%      | 0%        |
| 200000GBP  | false      | true       | 2%      | 5%        |
| 2000000GBP | false      | false      | 12%     | 0%        |
```

**この例が見せていること**

- **軽減は途中で切れます。** 「£500,000 を超えると使えない」と書いてあるので、軽減の表は £500,000 までしか行を持ちません。その上では本則がそのまま顔を出します。`overrides` が「一部だけを覆う表」であるというのは、こういうことです。
- **最初の行にだけ出典が付いていません。** ページはその帯を「Zero」と書くだけで率を書いていないので、`0%` は規則側の書き方です。
- **第二の住宅の 5% は文章です。** 表ではないので、その行は文書を丸ごと引用しています（`@gov`）。帯の率と足し合わせていないのは、税額そのものが帯ごとの積み上げで、ページがその合計を表にしていないからです。

## 騒音にどれだけさらしてよいか

29 CFR 1910.95 の Table G-16。dBA の水準ごとに一日の許容時間が決まります。条文は eCFR から日付を指定して取ってきた写しに留めてあります。

```rule
rule osha_noise v1
description "The daily exposure to continuous noise a workplace may permit, from Table G-16 of 29 CFR 1910.95"

source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.95" sha256:f83a1303a004b5ae

inputs
  level : sound[dB]  range >=90dB <=130dB

outputs
  permitted : duration[min]  round down(1min)

# Table G-16 lists nine levels and the time permitted at each: 8 hours at 90 dBA, 6 at 92,
# and so on down to a quarter of an hour at 115. A level between two of the listed ones is
# read here as **the shorter of the two** — 91 dBA is given the 92 dBA row — and that is a
# decision made here, not in the text. The appendix says the reference duration "is computed
# by" a formula, but the formula is a picture in the document and no text of it comes out of
# the copy; rounding the other way would permit longer exposure than the formula does, which
# is the wrong direction to be wrong in.
table exposure  @osha "§1910.95"
policy first
| level          | -> permitted : duration[min] |
| <=90dB         | 480min                       |
| >90dB <=92dB   | 360min                       |
| >92dB <=95dB   | 240min                       |
| >95dB <=97dB   | 180min                       |
| >97dB <=100dB  | 120min                       |
| >100dB <=102dB | 90min                        |
| >102dB <=105dB | 60min                        |
| >105dB <=110dB | 30min                        |
| -              | 15min                        |

examples
| level | -> permitted |
| 90dB  | 480min       |
| 92dB  | 360min       |
| 95dB  | 240min       |
| 100dB | 120min       |
| 105dB | 60min        |
| 110dB | 30min        |
| 115dB | 15min        |
| 91dB  | 360min       |
```

**この例が見せていること**

- **表に無い水準をどう読むかは、こちらで決めています。** 91 dBA は 92 dBA の行として読む — つまり短いほうの時間を採ります。附録は許容時間が式で計算されると書いていますが、その式は文書の中では画像で、写しの文字には出てきません。逆に丸めると式より長くさらしてよいことになるので、そちらには倒しません。
- **音は比較と範囲だけの型です。** `sound[dB]` は足し算ができません。デシベルは対数なので、二つ足しても二つぶんの音にならないからです。
- **時間は分で持っています。** 表に 1½ 時間と ¼ 時間があるので、時間単位では整数になりません。`duration[min]` なら 90 分・15 分とそのまま書けます。

## 掘削に防護が要るかどうか

29 CFR 1926.652(a)(1)。同じ title の別の part なので、出典も別に宣言しています。答えは金額ではなく「要る／要らない」です。

```rule
rule osha_excavation v1
description "Whether an excavation needs a protective system against cave-ins, from 29 CFR 1926.652(a)(1)"

# A second part of the same title, and so a source of its own: the id names the part, and
# `rulec source fetch` brings the section from the eCFR as of the date on the line.
source osha = law ecfr "29 CFR 1926" asof 2026-01-01
  "§1926.652" sha256:088a630a1ae9a4d7

enum verdict = required | not_required

# The section gives two exceptions. One is that the excavation "are made entirely in stable
# rock". The other is two conditions at once: less than five feet deep, **and** an
# examination by a competent person giving "no indication of a potential cave-in". What the
# rule asks for is the examination's answer, not whether one was made — no examination is not
# the same as one that found nothing, and the row for it is the one that requires the system.
inputs
  depth              : length[ft]  range >=1ft <=30ft
  stable_rock        : bool
  cave_in_indication : bool

outputs
  protection : verdict

table needed  @osha "§1926.652"
policy unique
| stable_rock | depth | cave_in_indication | -> protection : verdict |
| true        | -     | -                  | not_required            |
| false       | <5ft  | false              | not_required            |
| false       | <5ft  | true               | required                |
| false       | >=5ft | -                  | required                |

examples
| depth | stable_rock | cave_in_indication | -> protection |
| 4ft   | false       | false              | not_required  |
| 4ft   | false       | true               | required      |
| 5ft   | false       | false              | required      |
| 12ft  | true        | true               | not_required  |
```

**この例が見せていること**

- **例外は二つで、片方は条件が二つあります。** 岩盤だけを掘るとき、または「5 フィート未満で、資格のある者が見て崩落の兆候が無いとき」。後者が二列になっているのは、条文がそう書いているからです。
- **「調べていない」は「調べて何も無かった」ではありません。** だから入力は調査の結果であって、調査をしたかどうかではありません。調べていない現場は、兆候ありの行に落ちます。
- **フィートで書いてあります。** 条文が「5 feet (1.52m)」と書くので、そのまま `length[ft]` です。

## PayPal の決済手数料

米国の PayPal Checkout の手数料です。公開されている料金表からの転記で、法令ではなく事業者の規約を写した例。`決済手数料.rule` の英語圏版にあたります。

```rule
rule paypal_fee v1
description "The PayPal Checkout fee on one payment in the United States. Transcribed from PayPal's published merchant fees"

source paypal = file "sources/paypal-us-fees.md" sha256:0318950a982c3c7d  # PayPal's own published figures
  table1 sha256:5e481ef40e570eeb

inputs
  amount        : money[USDc]  range >=1USDc <=100000000USDc
  international : bool

# The fee has fractions of a cent in it, and the page does not say which way they settle, so
# the direction here is a placeholder — the thing a person has to decide before this ships.
outputs
  fee : money[USDc]  round half_up(1USDc)

# The page prints the domestic rate and, separately, what an international transaction adds.
# It does not print the sum, so neither does this: the row for a domestic payment adds
# nothing, and that row carries no citation because `0%` is not a figure the copy shows.
table surcharge
policy unique
| international | -> extra : rate[step 0.01%] |
| false         | 0%                          |
| true          | 1.5%                        |  @paypal table1

define fee : money[USDc] = amount × 3.49% + amount × extra + 49USDc  @paypal table1

examples
| amount    | international | -> fee  |
| 10000USDc | false         | 398USDc |
| 10000USDc | true          | 548USDc |
```

**この例が見せていること**

- **率と定額の両方が出典から来ています。** 3.49% と 0.49 ドル。合計の率（4.99%）はページに無いので、こちらでも作りません。国際取引の 1.5% は別の行です。
- **丸めの向きは仮置きです。** セント未満が出るのにページが何も言っていないので、`half_up` と書いたうえで「これは決め事の置き場所だ」と注に書いてあります。決めるのは人です。
- **セントで数えています。** `money[USDc]` はセントの整数で、$0.49 は 49USDc。ドルとセントが取り違えられることはありません。

## 領収書の印紙税

国税庁タックスアンサー No.7141 の第17号文書（売上代金に係る金銭又は有価証券の受取書）の税額表です。受取金額のほかに、金額の記載があるか、営業に関するものかで決まります。

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

**この例が見せていること**

- **非課税は 0 円の行です。** 5 万円未満と、営業に関しないものは非課税で、表はそれを `0円` の行として持ちます。「課税されない」も規則の答えのひとつです。
- **金額の記載が無いものは金額を見ません。** `金額の記載あり` が false の行は受取金額の列が `-` で、いくらでも 200 円です。三つの入力のどの組み合わせもちょうど一行に当たることを、検査が証明しています。

## 所得税の速算表

国税庁タックスアンサー No.2260 の速算表です。課税される所得金額の段ごとに税率と控除額があり、`課税所得 × 税率 − 控除額` で所得税が出ます。復興特別所得税はその 2.1% です。

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

**この例が見せていること**

- **一つの表が率と金額を同時に出します。** 税率の列は `rate[step 1%]`、控除額の列は `money[円]` で、`define` がその二つを掛けて引きます。ページの計算例（7,000,000円 × 0.23 − 636,000円 = 974,000円）が、そのまま `examples` の行です。
- **段の境目は「次の段の始まり未満」で書いています。** ページは「1,000円 から 1,949,000円まで」「1,950,000円 から」と書きますが、課税所得は 1,000円 単位なので同じことです。整数の全体で完全性を証明するには、隙間の無い書き方のほうが要ります。
- **書いていないことは仮置きと明記します。** 復興特別所得税に 1 円未満の端数が出たときの扱いは、このページにはありません。`round down` に仮置きして、宣言の横のコメントに出典が無いことを残しています。承認する人は `rulec doc` でそれを読みます。

## 契約書の印紙税と、期限つきの軽減税率

不動産の譲渡に関する契約書（第1号文書）の印紙税額です。本則（No.7140）と、令和9年3月31日までに作成された契約書の軽減税率（No.7108）を一つの表に持ちます。作成日が入力です。

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

**この例が見せていること**

- **期限のある特例は、日付の定義と一つの列になります。** `define 軽減期間 = 作成日 <= 2027-03-31` を列に置き、軽減の行は `true`、本則の行は `false`、期間によらない行は `-` です。`policy unique` なので、どの契約金額もどの作成日も、ちょうど一行に当たることが証明されています。
- **軽減の対象外は本則の行が受けます。** 軽減は契約金額が 10 万円を超えるものだけなので、1 万円未満（非課税）と 10 万円以下の行は、期間の列が `-` です。
- **この規則が生成器の欠陥を一つ見つけました。** 定義の中の日付リテラルが、全言語で 0 として生成されていました。参照評価器は正しく読んでいたので、`rulec test` の突き合わせで食い違いとして出ました。

## 厚生年金保険料の等級表

日本年金機構の厚生年金保険料額表（令和8年度版）です。32 等級で、料率は一般の被保険者なら 18.3% ですが、厚生年金基金の加入員は基金ごとに違うので入力にしてあります。

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

**この例が見せていること**

- **健康保険料と同じ形です。** 表が 50 等級から 32 等級になり、上限が 650,000 円になるだけで、折半と二つの端数処理はそのままです。同じ形の規則は、同じ形に写せます。
- **この表では二つの端数処理が同じ答えになります。** 18.3% × 標準報酬月額は必ず偶数の円なので、折半額に端数が出ません。規則には二つの丸め方が書いてありますが、この料率では表に現れない、ということまで `examples` が示しています。
- **印刷された 32 等級すべてと突き合わせてあります。** 表の折半額を写した記録（`tests/oracle/`）に `rulec replay` を当て、全件一致することをテストが確かめます。
- **写し元の Excel に縛ってあります。** `source` が日本年金機構の保険料額表（`.xlsx`）そのものを指し、`@機構 表1` がその一枚目のシートを引きます。`rulec source fetch` がシートを取り出して規則の隣に置き、以後 `rulec check` は、**この表の 32 個の標準報酬月額がその写しに出てくる値であること**を確かめます（E116）。`470000円` を `480000円` と写せば、刻みにも載っていて抜けも重なりもないのに、そこだけが落ちます。

## 保険料額表を写して、二つの端数処理を出す

協会けんぽの保険料額表（東京支部、令和8年3月分から）です。報酬月額から 50 等級の標準報酬月額を引き、料率を掛けて折半します。円未満の端数は、給与から控除するときと現金で納めるときで丸め方が違います。この表を写すために `half_down` が言語に入りました。

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

**この例が見せていること**

- **同じ折半額から、丸め方の違う二つの出力を返します。** 表の注記は、給与から控除するなら「50銭以下は切り捨て、50銭を超える場合は切り上げ」、現金で納めるなら「50銭未満は切り捨て、50銭以上は切り上げ」と書いています。後者は `half_up`、前者は `half_down` です。折半額が 6,599.5 円の等級で、二つの出力は 1 円違います。
- **料率は入力です。** 都道府県ごと、年度ごとに変わるものを規則に焼き込むと、改定のたびに表を書き換えることになります。`derive` で健康保険料率と介護保険料率を足し、介護保険第2号被保険者かどうかで、表が使う率を選びます。
- **表の全等級と突き合わせてあります。** 印刷された折半額を写した記録（`tests/oracle/`）に `rulec replay` を当て、100 件すべてで一致することをテストが確かめます。

## 本則と特例を二つの表に分け、出典に縛る

上の印紙税と同じ決まりを、印紙税法の別表第一（本則）と、租税特別措置法第 91 条（軽減）の二つの表に分け、非課税の決まりを節にしたものです。それぞれの表が自分の出典を引用し、出典は政府の法令データベース（e-Gov 法令検索）から取った条文の写しのハッシュに縛られています。

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

**この例が見せていること**

- **`overrides 本則` が、特例を本則に優先させます。** 一つの表に `軽減期間` の列を足す代わりに、原文ごとに表を分けて、どちらが勝つかを一行で書きます。検査は二つの表をまとめて、完全性と重なりを見ます。
- **非課税の決まりは `clause` です。** 別表第一では、非課税の物件は課税物件の表の外の欄に書かれているので、表の行にはせず文のまま書いています。
- **`source` と `@` が出典を縛ります。** `rulec source fetch` が e-Gov から別表第一と第 91 条の写しを取り、`rulec source pin` がハッシュを書きます。条文が変われば、その箇所を引用している表を名指しして止まります（E038）。
- **行のラベル**（`r1` …）は、記録に出る名前であり、`overrides 本則:r3` のように行を指す名前です。

## ただし書を、文のまま書く

運賃表が基本運賃を決め、送料は二つの節で決まります。本文の「通常」と、会員の 3,900 円以上の注文を無料にするただし書です。ただし書は条件が列に並ばないので、表ではなく `clause` で書いています。

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

**この例が見せていること**

- **`clause` は一行の表です。** `when` に条件、`then` に値。検査も生成も記録も表と同じで、記録には `{"table":"無料","row":1}` と出ます。
- **`overrides 通常` で、ただし書が本文に優先します。** 承認用の資料は「節 無料 は 節 通常 に優先する。交わる 1 対のすべてで、無料の行は通常の行に収まる（例外）」と書きます。
- **別名の無い群**（`group 遠隔地 = 北海道, 沖縄県`）も書けます。生成コードでは `g1` のような番号つきの名前になります。

## 準用される側の規則

次の例が準用する元の規則です。勤続年数と退職事由で支給月数を決め、自己都合退職の減額の節が本則に優先します。実在の法令ではなく、設計文書の例を規則にしたものです。

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

**この例が見せていること**

- **この規則は単独で検査され、単独で生成できます。** 準用する側は、このファイルのハッシュを見出しに書いて縛ります。
- **`減額` の節は、準用する側が `except` で外せます。** 「第 20 条（第 2 項を除く。）の規定は…準用する」の形です。

## ほかの規則を読み替えて準用する

上の退職手当の規則を、非常勤職員に準用します。「勤続年数」を「在職期間」と、「退職事由」を「任期終了事由」と読み替え、減額の節は準用しません。

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

**この例が見せていること**

- **読み替えは `<元の規則の入力> = <この規則の値>` です。** 列挙どうしは `with 任期満了 -> 定年, 辞職 -> 自己都合` で値を対応づけます。
- **渡す値が元の規則の範囲に収まることを check が確かめます（E043）。** 在職期間は 1〜3 年で、勤続年数の 1〜40 年に収まります。0 から書けば、その値を例に挙げて止まります。
- **元の規則の表は、この規則の中に展開されて検査・生成されます。** 記録は `{"table":"退職手当:支給表","row":1,"label":"短期"}` と、元の表の名前で返ります。勤続年数 10 年以上の行はこの規則では当たらないので、エラーにはせず、承認用の資料に「この準用では当たらない行」として挙がります。
- **元の規則が変われば E040 で止まります。** `rulec diff` で何件いくら動くかを見て、それでよければ `rulec source pin` でハッシュを書き直します。

## 温度と容量で、貼る表示を決める

食品の保存基準を写したものです。摂氏の温度、ミリリットルの容量、そして「まだ決まっていない」を持てる列が一度に出てきます。

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
| 表示区分 | 保存温度     | -> 適温(in_range) : bool |
| 冷凍     | <=-15℃      | true                     |
| 冷凍     | >-15℃       | false                    |
| 冷蔵     | >=0℃ <=10℃ | true                     |
| 冷蔵     | <0℃         | false                    |
| 冷蔵     | >10℃        | false                    |
| 常温     | -            | true                     |

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
| false | -      | >25℃    | 廃棄                    | "廃棄"              |
| false | -      | -        | 要冷却                  | "要冷却"            |

examples
| 保存温度 | 容量   | 表示区分 | 再検区分 | -> 判定 | 表示             |
| -20℃    | 500mL  | 冷凍     | none     | 適合    | "適合"           |
| -10℃    | 500mL  | 冷凍     | 冷凍     | 要冷却  | "要冷却・小分け" |
| 5℃      | 1000mL | 冷蔵     | none     | 適合    | "適合"           |
| 30℃     | 2000mL | 冷蔵     | 冷蔵     | 廃棄    | "廃棄"           |
| 15℃     | 500mL  | 冷蔵     | none     | 要冷却  | "要冷却"         |
```

**この例が見せていること**

- **温度は比べるだけの型です。** `℃` は 0 が「無い」を意味しない目盛りなので、足し算も倍にすることもできません。書けるのは比較と `range` だけで、`気温 - 気温` は E048 で止まります。
- **`区分?` は、値がまだ無いことを持てる列です。** セルの `none` でだけ受けられて、式には出てきません。null を算術から締め出すために、表で場合分けさせる形になっています。
- **出力は文字列でもかまいません。** 貼る表示のように、そのまま人が読む値です。ただし文字列は表の入力の列にはできません（E110）——分岐を決める値は列挙にします。

## 面積と騒音と残業から、措置と負担を決める

事務所衛生基準規則と騒音障害防止のスケッチです。面積・音量・時間という三つの次元と、入力どうしの関係の宣言が出てきます。

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

**この例が見せていること**

- **`constraint 占有面積 <= 床面積` は、呼び出し側が守ると約束した関係です。** 検査はその外側に行を求めず、診断が返す入力は必ず実在しうる一件になり、生成コードは違反する入力を入口で断ります。
- **面積は長さの積ではなく、独立した次元です。** `縦 × 横` は E103 で止まります——この道具は次元解析をしないので、積を入れる次元を黙って作ることはしません。
- **音量の dB も比べるだけです。** 対数の目盛りなので、二つ足しても音が二つ分にはなりません。
- **`round half_even` は五つの丸めのうちの一つです。** 同点を偶数側へ寄せる、会計でよく使われる向きです。

---

[表(.rule)を書く](tour.md){ .md-button .md-button--primary }
[文法](reference.md){ .md-button }
