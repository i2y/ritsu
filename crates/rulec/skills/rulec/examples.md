# Examples

Every rule on this page is **one the repository's tests run on every commit**: it passes
`rulec check`, its own examples execute, and the reference evaluator and every generated
language are held to the same answers byte for byte. Copy any of them and it works.

They are ordered smallest first.

**All of them are written in English.** Some began that way — [whether a return is
accepted](#whether-a-return-is-accepted-in-english), [a parcel tariff in pounds and
inches](#a-parcel-tariff-in-pounds-and-inches), [Article 7 of Regulation (EC) No
261/2004](#a-rule-written-in-english--eu-air-passenger-rights), [the UK minimum
wage](#a-minimum-wage-and-the-exception-that-overrides-it), [the UK personal
allowance](#a-personal-allowance-that-tapers-and-the-band-above-it), [the US federal income
tax](#the-us-federal-income-tax-bracket-by-bracket), [one section of the
CFR](#one-section-of-the-us-code-of-federal-regulations) and the four after it. The rest began
as transcriptions of Japanese published terms and statutes, and each is shown here as its
English twin: the same rule with English names, `JPY` for the yen, and the amounts written out
in digits. What stays in Japanese in them belongs to the documents: the headings that a
`source` line points at in a statute or a workbook. The prefectures are spelled in English,
from `std/jp/prefectures`.
[The Japanese page](https://i2y.github.io/rulec/ja/examples/) shows the originals, with their
names in Japanese, and the repository's tests hold each pair to the same findings, the same
answers and the same claims.

## A date decides which period it is

The smallest example there is: one input, one output. The rows are adjacent date ranges laid end to end with nothing between them.

```rule
rule order_period v1
description "Decides the period in force from the order date. An example of date ranges, both ends included, laid side by side with no gap between them"
# Were dates not held as day numbers, a gap of integers that does not exist would open between the end of a month and the start of the next, and a false E101 would appear (§2.1)

enum period = before | spring | normal | year_end

inputs
  order_date : date  range >=2026-01-01 <=2026-12-31

outputs
  kind : period

table pick
policy unique
| order_date                | -> kind : period |
| <=2026-03-31              | before           |
| >=2026-04-01 <=2026-06-30 | spring           |
| >=2026-07-01 <=2026-11-30 | normal           |
| >=2026-12-01              | year_end         |

examples
| order_date | -> kind  |
| 2026-03-31 | before   |
| 2026-04-01 | spring   |
| 2026-06-30 | spring   |
| 2026-07-01 | normal   |
| 2026-12-01 | year_end |
```

**What this one shows**

- A date has **comparison and range, and nothing else** — no addition, no subtraction.
- The checker knows `<=2026-03-31` and `>=2026-04-01` are adjacent, because a date is held internally as a day number. Held as `20260331` instead, there would be a phantom gap between the end of one month and the start of the next, and the completeness check would report a hole that is not there.
- The output is not a number, so no `round` is required.

## A shipping fee with three conditions crossing

The design sketch written out as a rule. A boolean definition used as a column, a rate output, the `default` mark and `policy first` shadowing all appear at once.

```rule
rule member_shipping_fee v4
description "The sketch of §1.2 of the design document, written as a rule. It shows the normal case of a boolean definition used as a column, a rate output, the default mark, and one pair of rows that policy first leaves for review"

import std/jp/prefectures

enum member_kind = basic default | gold default | platinum
group remote = Hokkaido, Okinawa

inputs
  dest   : jp_prefecture
  weight : mass[g]  range >=1g <=40kg
  total  : money[JPY, incl_tax]  range >=0JPY <=10_000_000JPY
  member : member_kind

outputs
  fee : money[JPY, incl_tax]  round up(10JPY)

# A boolean definition. The condition reads one input only, so expanded it is a sum of regions (§5.3, §6.2)
define bulk : bool = total >= 30_000JPY

table base
policy unique
| dest        | weight  | -> base : money[JPY, incl_tax] |
| remote      | <=2000g | 1200JPY                        |
| remote      | >2000g  | 1800JPY                        |
| not: remote | <=2000g | 800JPY                         |
| not: remote | >2000g  | 1100JPY                        |

table payer
policy first
| bulk | member   | -> pay_rate : rate |
| true | -        | 0%                 |
| -    | platinum | 50%                |
| -    | -        | 100%               |

result fee = base * pay_rate

examples
| dest     | weight | total     | member   | -> fee  |
| Okinawa  | 2500g  | 40_000JPY | basic    | 0JPY    |
| Tokyo    | 1999g  | 12_000JPY | platinum | 400JPY  |
| Hokkaido | 500g   | 5000JPY   | basic    | 1200JPY |
```

**What this one shows**

- `define … : bool` is **a named boolean you can put in a column**. Naming the condition is what makes the table readable.
- `default` declares that a value needs no row of its own. Without it you get "appears in no row".
- Under `policy first` an earlier row hides a later one. The checker sorts those into three kinds and counts them, so the staircase does not drown the one pair that matters.

## Transcribing a real published tariff

Japan Post's base tariff, shipping from Tokyo. 47 prefectures × 7 sizes = 329 combinations, folded into 42 rows by six groups. **This is where the tool first pays for itself**: drop one prefecture and it stops before anything runs, naming that prefecture.

```rule
rule yupack_base_fee v1
description "The base fee for sending from Tokyo"

import std/jp/prefectures

enum size_class = S60 | S80 | S100 | S120 | S140 | S160 | S170

group tokyo   = Tokyo
group hk      = Hokkaido, Fukuoka, Saga, Nagasaki, Kumamoto, Oita, Miyazaki, Kagoshima
group near    = Aomori, Iwate, Miyagi, Akita, Yamagata, Fukushima, Ibaraki, Tochigi, Gunma, Saitama, Chiba, Kanagawa, Yamanashi, Niigata, Nagano, Toyama, Ishikawa, Fukui, Gifu, Shizuoka, Aichi, Mie
group kinki   = Shiga, Kyoto, Osaka, Hyogo, Nara, Wakayama
group cs      = Tottori, Shimane, Okayama, Hiroshima, Yamaguchi, Tokushima, Kagawa, Ehime, Kochi
group okinawa = Okinawa

inputs
  dest   : jp_prefecture
  girth  : length[cm]  range >=1cm <=170cm
  weight : mass[g]  range >=1g <=25kg  contract_only

outputs
  fee : money[JPY, incl_tax]  round up(10JPY)

table size_of  # Source: Japan Post, base fee table (Tokyo), size classes
policy first
| girth   | -> size : size_class |
| <=60cm  | S60                  |
| <=80cm  | S80                  |
| <=100cm | S100                 |
| <=120cm | S120                 |
| <=140cm | S140                 |
| <=160cm | S160                 |
| -       | S170                 |

table fee_table  # Source: Japan Post, base fee table (Tokyo)
policy unique
| dest    | size | -> fee : money[JPY, incl_tax] |
| tokyo   | S60  | 820JPY                        |
| tokyo   | S80  | 1130JPY                       |
| tokyo   | S100 | 1450JPY                       |
| tokyo   | S120 | 1770JPY                       |
| tokyo   | S140 | 2120JPY                       |
| tokyo   | S160 | 2450JPY                       |
| tokyo   | S170 | 3000JPY                       |
| near    | S60  | 880JPY                        |
| near    | S80  | 1200JPY                       |
| near    | S100 | 1500JPY                       |
| near    | S120 | 1830JPY                       |
| near    | S140 | 2170JPY                       |
| near    | S160 | 2500JPY                       |
| near    | S170 | 3070JPY                       |
| kinki   | S60  | 990JPY                        |
| kinki   | S80  | 1310JPY                       |
| kinki   | S100 | 1620JPY                       |
| kinki   | S120 | 1940JPY                       |
| kinki   | S140 | 2300JPY                       |
| kinki   | S160 | 2610JPY                       |
| kinki   | S170 | 3750JPY                       |
| cs      | S60  | 1150JPY                       |
| cs      | S80  | 1440JPY                       |
| cs      | S100 | 1780JPY                       |
| cs      | S120 | 2080JPY                       |
| cs      | S140 | 2440JPY                       |
| cs      | S160 | 2750JPY                       |
| cs      | S170 | 3890JPY                       |
| hk      | S60  | 1410JPY                       |
| hk      | S80  | 1710JPY                       |
| hk      | S100 | 2020JPY                       |
| hk      | S120 | 2340JPY                       |
| hk      | S140 | 2680JPY                       |
| hk      | S160 | 3010JPY                       |
| hk      | S170 | 4140JPY                       |
| okinawa | S60  | 1450JPY                       |
| okinawa | S80  | 1810JPY                       |
| okinawa | S100 | 2160JPY                       |
| okinawa | S120 | 2490JPY                       |
| okinawa | S140 | 2860JPY                       |
| okinawa | S160 | 3180JPY                       |
| okinawa | S170 | 4350JPY                       |

examples
| dest     | girth | weight | -> fee  |
| Tokyo    | 55cm  | 1kg    | 820JPY  |
| Okinawa  | 100cm | 3kg    | 2160JPY |
| Hokkaido | 61cm  | 20kg   | 1710JPY |
```

**What this one shows**

- A `group` names part of an enum. It is always expanded back to the values for checking, so **whether the grouping is an exact partition of the 47** is checked too.
- `import std/jp/prefectures` brings the 47 values in, spelled in English (`Tokyo`). The original imports the same 47 under the namespace's Japanese name, spelled in Japanese; either import reads both spellings, and the generated code is the same.
- 42 rows, and `policy unique` still proves **reordering them cannot change the answer**.

## Two outputs at once

For one coupon: whether it applies, and how much it takes off. Stacking several coupons — the order and the loop — stays with the caller.

```rule
rule single_coupon v1
description "Whether one coupon applies, and its raw discount (the sketch of §5.4 of the design document). The order of stacking and the loop are the caller's. It has several outputs, and the names of output cells appear in it"

enum coupon_kind = percent | fixed | free_ship

inputs
  subtotal : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  applied  : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  kind     : coupon_kind
  rate     : rate[step 1%]  range >=0% <=100%
  face     : money[JPY, incl_tax]  range >=0JPY <=100_000JPY
  dup      : bool

outputs
  ok  : bool
  raw : money[JPY, incl_tax]  round down(1JPY)

# The public terms say nothing about rounding, so round down is a placeholder (§7.1)
derive margin : money[JPY, incl_tax] = subtotal - applied - face  range >=-1_100_000JPY <=1_000_000JPY

define rate_off : money[JPY, incl_tax] = subtotal * rate

table applicable
policy first
| kind  | margin | dup  | -> ok : bool |
| -     | -      | true | false        |
| fixed | <=0JPY | -    | false        |
| -     | -      | -    | true         |

table raw_discount
policy unique
| ok    | kind      | -> raw : money[JPY, incl_tax] |
| false | -         | 0JPY                          |
| true  | percent   | rate_off                      |
| true  | fixed     | face                          |
| true  | free_ship | 0JPY                          |

examples
| subtotal  | applied | kind      | rate | face   | dup   | -> ok | raw     |
| 10_000JPY | 0JPY    | percent   | 10%  | 0JPY   | false | true  | 1000JPY |
| 10_000JPY | 0JPY    | fixed     | 0%   | 500JPY | false | true  | 500JPY  |
| 400JPY    | 0JPY    | fixed     | 0%   | 500JPY | false | false | 0JPY    |
| 10_000JPY | 0JPY    | percent   | 10%  | 0JPY   | true  | false | 0JPY    |
| 10_000JPY | 0JPY    | free_ship | 0%   | 0JPY   | false | true  | 0JPY    |
```

**What this one shows**

- **There can be two or more outputs.** Two tables fill one each, and each carries its own `round`.
- An output cell holds **one value or one name**. The arithmetic moves to a `define`, so the table keeps the branching and nothing else.
- The first table's output (`ok`) is a column of the second. Items run one way, top to bottom, so the dependencies are always readable off the page.

## Eligibility and amount together

The same subject in another shape: a `derive` computes what is left, and that becomes a column.

```rule
rule coupon_discount_amount v1
description "Whether one coupon applies, and how much it takes off. The order and the loop are the caller's. Source: reconstructed from the public help pages of Rakuten and Yahoo"
# Note: an output cell may carry a name (adopted in §3.2). An expression cannot be written there, so a rate discount goes through a define

enum coupon_kind  = percent | fixed | free_ship
enum coupon_scope = shop default | item

inputs
  list         : money[JPY,incl_tax]  range >=0JPY <=10_000_000JPY  # the basis of a rate coupon
  running      : money[JPY,incl_tax]  range >=0JPY <=10_000_000JPY  # the basis of an amount coupon (after the rate coupon has applied)
  kind         : coupon_kind
  scope        : coupon_scope
  rate         : rate[step 1%]
  face         : money[JPY,incl_tax]  range >=0JPY <=1_000_000JPY
  cap          : money[JPY,incl_tax]  range >=0JPY <=1_000_000JPY
  min_purchase : money[JPY,incl_tax]  range >=0JPY <=1_000_000JPY
  used         : bool
  alive        : bool

outputs
  discount : money[JPY,incl_tax]  round down(1JPY)

# [derive] the shape the real table asked for. Both are linear combinations of the form "input - input"
derive min_gap : money[JPY,incl_tax] = running - min_purchase  range >=-1_000_000JPY <=10_000_000JPY
derive pay_gap : money[JPY,incl_tax] = running - face          range >=-1_000_000JPY <=10_000_000JPY

table applicable
policy first
| alive | used | scope | kind  | min_gap | pay_gap | -> ok : bool |
| false | -    | -     | -     | -       | -       | false        |
| -     | true | item  | -     | -       | -       | false        |
| -     | -    | -     | -     | <0JPY   | -       | false        |
| -     | -    | -     | fixed | -       | <=0JPY  | false        |
| -     | -    | -     | -     | -       | -       | true         |

# A rate coupon is based on the list price, an amount coupon on what is left after the discount (the running balance), as Rakuten's text has it
define rate_off : money[JPY,incl_tax] = list * rate

table raw_discount
policy unique
| ok    | kind      | -> raw : money[JPY,incl_tax] |
| false | -         | 0JPY                         |
| true  | percent   | rate_off                     |
| true  | fixed     | face                         |
| true  | free_ship | 0JPY                         |

result discount = down(min(raw, cap), 1JPY)

examples
| list      | running   | kind    | scope | rate | face   | cap     | min_purchase | used  | alive | -> discount |
| 10_000JPY | 10_000JPY | percent | shop  | 10%  | 0JPY   | 1000JPY | 5000JPY      | false | true  | 1000JPY     |
| 10_000JPY | 9000JPY   | fixed   | shop  | 0%   | 500JPY | 500JPY  | 5000JPY      | false | true  | 500JPY      |
| 10_000JPY | 400JPY    | fixed   | shop  | 0%   | 500JPY | 500JPY  | 0JPY         | false | true  | 0JPY        |
```

**What this one shows**

- A `derive` is a named amount built from **additions and subtractions of inputs only**. It is the one intermediate value that can sit in a column as a quantity, and it must declare a `range`.
- If the declared range does not contain the values that can actually occur, E112 stops it — the range is the universe the checks reason over.

## When the checker could not decide

Two derived values share an input, so whether two rows can fire together is not decidable here. The tool neither waves it through nor invents an error: **it warns, and puts a runtime guard in the generated code.**

```rule
rule coupon_stacking v1
description "A unique table whose two derives share inputs. It looks overlapping to the sieve that reads one column at a time, and is the subject for deciding that no elimination runs (§6.2, §15.126)"

enum stack_verdict = no default | yes

inputs
  total  : money[JPY,incl_tax]  range >=0JPY <=1_000_000JPY
  disc_a : money[JPY,incl_tax]  range >=0JPY <=100_000JPY
  disc_b : money[JPY,incl_tax]  range >=0JPY <=100_000JPY

outputs
  verdict : stack_verdict

# disc_b is at least 0 yen, so by its definition rest_b is never above rest_a.
# So when rest_a <= 1000 yen, rest_b cannot reach 3980 yen.
# The check reads the derives one at a time, so it cannot see this link. Rows 1 and 2 below
# look overlapping there, and Fourier-Motzkin elimination is decided not to run (§15.126).
derive rest_a : money[JPY,incl_tax] = total - disc_a           range >=-100_000JPY <=1_000_000JPY
derive rest_b : money[JPY,incl_tax] = total - disc_a - disc_b  range >=-200_000JPY <=1_000_000JPY

table decide
policy unique
| rest_a    | rest_b    | -> verdict : stack_verdict |
| <=1000JPY | -         | no                         |
| -         | >=3980JPY | yes                        |
| >1000JPY  | <3980JPY  | no                         |

examples
| total     | disc_a  | disc_b  | -> verdict |
| 500JPY    | 0JPY    | 0JPY    | no         |
| 10_000JPY | 1000JPY | 1000JPY | yes        |
| 5000JPY   | 2000JPY | 2000JPY | no         |
```

**What this one shows**

- W114 says "not proved". It does not say the overlap exists, and it does not say it doesn't.
- The generated code refuses to silently pick the earlier row if such an input ever arrives; it raises instead. **If that guard ever fires, the overlap was real.**
- This is the only place where something undecided statically is carried into runtime.

## The answer is an order

Which of two coupons applies first. The sorting itself is the caller's loop, but **the comparison lives in the rule**, because that is the part someone has to approve.

```rule
rule coupon_order v1
description "Which of two coupons is applied first (§5.4). The loop and the sorting are the caller's; only the comparison is a rule"
# Rakuten's terms: rate coupons first, then amount coupons, and among amount coupons the one whose expiry is nearer first

enum kind  = percent | fixed
enum order = first | second

inputs
  a_kind : kind
  b_kind : kind
  a_due  : date
  b_due  : date

outputs
  a_order : order

# A comparison of two dates. A date cannot be a derive, so this is the second kind of atom, comparing two names directly (§5.3).
define a_earlier : bool = a_due <= b_due

table decide
policy first
| a_kind  | b_kind  | a_earlier | -> a_order : order |
| percent | fixed   | -         | first              |
| fixed   | percent | -         | second             |
| -       | -       | true      | first              |
| -       | -       | -         | second             |

examples
| a_kind  | b_kind  | a_due      | b_due      | -> a_order |
| percent | fixed   | 2026-12-31 | 2026-01-01 | first      |
| fixed   | percent | 2026-01-01 | 2026-12-31 | second     |
| fixed   | fixed   | 2026-04-01 | 2026-05-01 | first      |
| fixed   | fixed   | 2026-05-01 | 2026-04-01 | second     |
| percent | percent | 2026-04-01 | 2026-04-01 | first      |
```

**What this one shows**

- Of the four kinds of answer — amount, yes/no, class, order — this is the fourth.
- The inputs are the attributes of both coupons and the output is "first" or "second", which keeps it to one decision.
- Two dates get compared here.

## Apportioning, one line at a time

Spreading one discount across the lines of an order. **One line is one decision**; the loop and the running remainder stay with the caller, who walks the lines subtracting as it goes. **The total comes out exact by construction** (checked over 2,000 generated sets of lines).

```rule
rule discount_allocation v1
description "Allocates a lump-sum discount to lines. One line is one decision, and the loop and the order are the caller's"

inputs
  list      : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  remaining : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  eligible  : bool

outputs
  applied : money[JPY, incl_tax]  round down(1JPY)

define cap : money[JPY, incl_tax] = min(list, remaining)

table applies
policy unique
| eligible | -> allocated : money[JPY, incl_tax] |
| true     | cap                                 |
| false    | 0JPY                                |

result applied = allocated

examples
| list    | remaining | eligible | -> applied |
| 3000JPY | 1000JPY   | true     | 1000JPY    |
| 500JPY  | 1000JPY   | true     | 500JPY     |
| 3000JPY | 1000JPY   | false    | 0JPY       |
```

**What this one shows**

- The rule decides how to allocate; the caller does the walking.
- `min` is available, so "up to this line's own value" fits on one line.
- Splitting by ratio is the other shape, written with `allocate` — "Apportioning by ratio" below.

## A rate finer than one percent

A per-contract fee rate turned into the fee on one transaction. The rate moves in tenths of a percent, so `0.5%` is a value you can write.

```rule
rule payment_processing_fee v1
description "The fee on one transaction, from a fee rate fixed per contract. An example where the rate comes in steps finer than 1%"

inputs
  amount : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  rate   : rate[step 0.1%]  range >=0% <=10%
  small  : bool

outputs
  fee : money[JPY, incl_tax]  round up(1JPY)

# A contractual floor: nothing is charged at 0.5% or below (this row can be written because the rate steps by 0.1%)
define rate_fee : money[JPY, incl_tax] = amount * rate

table fee_table
policy unique
| rate   | small | -> fee : money[JPY, incl_tax] |
| <=0.5% | -     | 0JPY                          |
| >0.5%  | true  | 50JPY                         |
| >0.5%  | false | rate_fee                      |

result fee = fee

examples
| amount    | rate | small | -> fee |
| 10_000JPY | 0.5% | false | 0JPY   |
| 10_000JPY | 0.6% | false | 60JPY  |
| 10_000JPY | 3.6% | false | 360JPY |
| 10_000JPY | 3.6% | true  | 50JPY  |
```

**What this one shows**

- `rate[step 0.1%]` declares that at runtime the value is a whole number of tenths of a percent: 0.5% travels as 5, 10% as 100.
- **A value between two steps cannot be written.** `0.05%` in that column stops at E114 — quietly moving it to the nearest step would put a different boundary in the code than the one on the page.
- The arithmetic stays a rate until the end, where it is rounded exactly once, because where to round is a business decision.

## Scores added up, then ranked by how much of the total they reach

No money anywhere in this one. Four scored criteria are added with weights, and the rank comes from how much of the maximum the total reaches. This is the shape of rule that is *not* written as a table today but becomes one the moment somebody writes it down.

```rule
rule rating_grade v1
description "Adds up four rating items with weights and decides the grade from the share of the full score they reach. An example with no money in it"

enum rank = S | A | B | C

inputs
  quality  : number  range >=0 <=10
  delivery : number  range >=0 <=10
  price    : number  range >=0 <=10
  support  : number  range >=0 <=10

outputs
  grade : rank

# The weights are 2:1:1:1, and the full score is 50
derive total : number = quality * 2 + delivery + price + support  range >=0 <=50
define ratio : rate = total / 50

table grade_table
policy first
| ratio | -> grade : rank |
| >=90% | S               |
| >=80% | A               |
| >=60% | B               |
| -     | C               |

result grade = grade

examples
| quality | delivery | price | support | -> grade |
| 10      | 10       | 10    | 10      | S        |
| 9       | 8        | 8     | 7       | A        |
| 6       | 6        | 6     | 6       | B        |
| 3       | 3        | 3     | 3       | C        |
```

**What this one shows**

- A `derive` is a name for **additions, subtractions and integer multiples of the inputs**, which is exactly what a weighted total is.
- Declaring `define ratio : rate = total / 50` lets the cells say `>=90%` — **the threshold stays a proportion on the page**. Whether a dimensionless value is called a rate or a number is the declaration's to say.
- **No division happens at runtime.** `>=90%` compiles to `total >= 45`, because a constant maximum folds the boundary into a constant. A maximum that is an *input* cannot be written: that is division by a variable, and E115 stops it — take the proportion itself as a `rate` input instead.

## Returning a number with no unit

One point per 100 yen. Dividing money by money cancels the unit and leaves a `number` — a whole number carrying none.

```rule
rule points_earned v1
description "The points earned by one purchase. One point per 100 yen, doubled by the member class and by the campaign. An example using a number with no unit"

enum member_kind = basic | gold

inputs
  paid     : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  kind     : member_kind
  campaign : bool

outputs
  pts : number  round down(1)

# One point per 100 yen. The fraction is dropped once, at the end (§7.2)
define base  : number = paid / 100JPY
define boost : number = base * 2

table pts_table
policy unique
| kind  | campaign | -> pts : number |
| gold  | -        | boost           |
| basic | true     | boost           |
| basic | false    | base            |

result pts = pts

examples
| paid    | kind  | campaign | -> pts |
| 1050JPY | basic | false    | 10     |
| 1050JPY | gold  | false    | 21     |
| 1050JPY | basic | true     | 21     |
| 99JPY   | basic | false    | 0      |
```

**What this one shows**

- `number` is the type for **counts, days and scores**: whole numbers with no unit.
- `paid / 100JPY` **does not divide the integer**. It multiplies the internal step by 100, so 1050 yen stays 10.5 points all the way to the end, where `round down(1)` makes it 10 exactly once. Python's `//` and Go's `/` truncate in different directions; neither gets a say here.
- Division is allowed **by a constant only**. If the divisor is business data, it belongs in the table as a rate or a constant column.

## Three tables stacked, two outputs returned

Weight gives a weight class, the class and the membership give a tier, and the tier with the amount payable gives shipping and a multiplier. What one table produces is a column of the next. Two things come back: the amount charged and the points.

```rule
rule member_benefits v1
description "Gives the amount billed and the points at once from the member class and the weight. Two outputs, one result, and a rate that comes from a column of a table"

enum member_class = basic | premium
enum weight_class = light | mid | heavy
enum tier_class   = bronze | silver | gold

inputs
  member : member_class
  weight : mass[g]  range >=1g <=20kg
  gross  : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  off    : money[JPY, incl_tax]  range >=0JPY <=100_000JPY

# The declaration order (billed, then points) and the alphabetical order (points, then billed) are reversed on purpose.
# Whether the generated code returns the outputs in the same order in every language shows only with this order.
outputs
  total : money[JPY, incl_tax]  round half_up(10JPY)
  pts   : number  round down(1)

derive net : money[JPY, incl_tax] = gross - off  range >=-100_000JPY <=1_000_000JPY

# Dividing an amount by an amount constant drops the unit and leaves a number (§2.3)
define base : number = gross / 100JPY

table weight_of
policy first
| weight | -> weight_band : weight_class |
| <=2kg  | light                         |
| <=10kg | mid                           |
| -      | heavy                         |

table tier_of
policy unique
| weight_band | member  | -> tier : tier_class |
| light       | basic   | bronze               |
| light       | premium | silver               |
| mid         | basic   | silver               |
| mid         | premium | gold                 |
| heavy       | -       | gold                 |

# One table gives two columns, and the define below uses the rate column.
# If the rate step (10%) did not fit the scale the column is kept at, the points would come out ten times too large.
table ship
policy unique
| tier   | net       | -> shipping : money[JPY, incl_tax] | multiplier : rate[step 10%] |
| bronze | <3000JPY  | 800JPY                             | 100%                        |
| bronze | >=3000JPY | 400JPY                             | 100%                        |
| silver | -         | 300JPY                             | 120%                        |
| gold   | -         | 0JPY                               | 150%                        |

# The second output is taken from the define with the same name as the output (result applies to the first output only)
define pts : number = base * multiplier

result total = max(net, 0JPY) + shipping

examples
| member  | weight | gross     | off     | -> total  | pts |
| basic   | 1kg    | 2000JPY   | 500JPY  | 2300JPY   | 20  |
| premium | 5kg    | 20_000JPY | 2000JPY | 18_000JPY | 300 |
| basic   | 15kg   | 5000JPY   | 0JPY    | 5000JPY   | 75  |
```

**What this one shows**

- **A table's output column is a column of any later table.** There is no limit on the depth (past the check's budget it stops at E109). When `rulec check` fails it names the row that fired in each of them: `table weight_of row 2 / table tier_of row 4 / table ship row 4`.
- **One table may produce several output columns.** `ship` produces `shipping` and `multiplier` at once, and the `define` below it uses that rate. A rate keeps its step (`step 10%`) through the column, so `base * multiplier` is rounded exactly once, at the end.
- **A `derive` can be a column.** Declaring `net = gross - off` turns judging on the amount after the discount into one column of `ship` rather than one bare line of arithmetic.
- **`result` assembles the first output and nothing else** (E015). The second and later ones are taken from a `define` of the same name - `define pts` here. A second `result` line stops at E016.

## A sequence walked into one answer

The caller passes the rows of a tariff sheet and the rule walks them in order. A table judges one element at a time, and `fold` says what each verdict does: go on, halt, take this one, hold the best so far. It is **the one shape that takes a number of things that is not fixed**.

```rule
rule nationwide_freight v1
description "Looks at the fare rows the caller passes, one by one, and folds them into a single fare. An example of a rule that walks a sequence"

enum pick = skip | halt | take | hold
enum zone = kinki | remote

# The fields of one row. The caller passes as many elements with these fields as it has
elements freight_rows
  row_zone  : zone
  threshold : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY
  row_fee   : money[JPY, incl_tax]  range >=0JPY <=100_000JPY

outputs
  fee : money[JPY, incl_tax]  round up(1JPY)

# The decision for one element. The checks of the table work as ever (these four rows cover verdict entirely)
table row_of
policy unique
| row_zone | threshold | -> verdict : pick |
| kinki    | <=1000JPY | take              |
| kinki    | >1000JPY  | hold              |
| remote   | <=1000JPY | skip              |
| remote   | >1000JPY  | halt              |

# Where each decision leads. Every decision needs a destination (E024)
fold verdict over freight_rows
  skip      -> next
  halt      -> stop with 0JPY
  take      -> take_unique row_fee
  hold      -> keep_max row_fee by threshold
  empty     -> 0JPY
  exhausted -> held

sequence near
| row_zone | threshold | row_fee |
| kinki    | 500JPY    | 800JPY  |
| kinki    | 2000JPY   | 1500JPY |

sequence no_rows
| row_zone | threshold | row_fee |

examples
| freight_rows | -> fee |
| near         | 800JPY |
| no_rows      | 0JPY   |
```

**What this one shows**

- **`elements` declares what one element carries.** The fields are declared exactly like `inputs`, ranges and units included, and the caller passes any number of elements with those fields filled in.
- **The table's own checks are unchanged.** One element is one case, so completeness, overlap and units are proved over it as they always were: the four rows above cover `pick` exactly.
- **The fold gives every verdict somewhere to go** — `next`, `stop with <value>`, `take_unique <value>` (a second element that also takes is a run-time error), `keep_max <value> by <key>`. A verdict with no arm stops at E024.
- **The answer for no elements, and for a walk that reached the end, are both required** (E022, E023). An empty sequence always turns up, and answering with what is held is a choice made by writing it (`exhausted -> held`).
- **An example names a `sequence`.** A cell holds one value, so the list is written under a name and the example points at it; a `sequence` with no rows is the example for a sequence with nothing in it.
- **SQL and NumPy are the two targets that do not get it.** One query has no place to carry a value from row to row and stop partway, and a walk that carries state from element to element is not a column operation. Every other target is generated, and agrees with the reference evaluator on every commit.

## Counting a sequence, and deciding from the count

An invoice's name is matched against the supplier ledger, one candidate at a time. A table judges each candidate, `count` counts the ones it called a match, and **the next table decides what to do with that number**. "One means automatic, more than one means look at it" is decided inside the rule rather than by whoever counted before calling it.

```rule
rule supplier_matching v1
description "Matches the addressee of an invoice against the supplier ledger one entry at a time, and decides the next step from the number of matches. An example of counting a sequence"

enum match_kind  = same | diff
enum action_kind = register | auto | review

inputs
  auto_ok : bool

# The candidates from the ledger. The fields of one candidate; the caller passes as many as it has
elements candidates
  name_match : bool
  addr_match : bool

outputs
  action : action_kind

# The decision for one candidate. The checks of the table work as ever
table row_of
policy unique
| name_match | addr_match | -> kind : match_kind |
| true       | true       | same                 |
| true       | false      | diff                 |
| false      | -          | diff                 |

# All that is left after the walk is the count. The range is the universe of the completeness check, and also the upper bound of the length of the sequence
count hits over candidates where kind = same  range >=0 <=50

table action_of
policy unique
| hits | auto_ok | -> action : action_kind |
| 0    | -       | register                |
| 1    | true    | auto                    |
| 1    | false   | review                  |
| >=2  | -       | review                  |

sequence one
| name_match | addr_match |
| true       | true       |
| true       | false      |

sequence two
| name_match | addr_match |
| true       | true       |
| true       | true       |

sequence no_candidates
| name_match | addr_match |

examples
| auto_ok | candidates    | -> action |
| true    | one           | auto      |
| false   | one           | review    |
| true    | two           | review    |
| true    | no_candidates | register  |
```

**What this one shows**

- **A count is what the walk leaves behind.** `count hits over candidates where kind = same` is how many elements the per-element table judged `same`. From there it is a `number`, so it can be a column.
- **What turns the number into a decision is an ordinary table.** A gap or an overlap in the `0` / `1` / `>=2` boundaries stops the check as it always would. Counting and deciding are checked separately.
- **The range says two things** (`range >=0 <=50`): the universe the completeness check quantifies over, and **the cap on the sequence**. Pass 51 candidates and the generated code refuses at the door — the same answer a number outside its range gets.
- **A count counts and a `sum` adds one column up** — "Adding the lines up" below. An average does not follow: dividing by the count is dividing by a variable, so compute one before the call.
- **A `fold` and a `count` cannot share a rule** (E031): two endings for the same walk, and a fold may stop partway.

## Adding the lines up

"Free shipping over 5,000 yen." The rule walks however many lines it is given, adds the amounts, and decides the fee from the total. Where `count` leaves a number behind, `sum` leaves an amount.

```rule
rule cart_shipping_fee v1
description "Decides the shipping fee from the sum of the amounts on the lines. An example of summing a sequence"

enum member_tier = regular | premium

inputs
  tier : member_tier

# One line of the order. The caller passes as many as it has
elements lines
  amount : money[JPY]  range >=0JPY <=100_000JPY

# All that is left after the walk is the total. The range is the universe of the completeness check, and it is also the line
# at which the entry refuses a call, as soon as the running total goes beyond it
sum total over lines of amount  range >=0JPY <=1_000_000JPY

outputs
  fee : money[JPY]  round up(1JPY)

table fee_table
policy unique
| total              | tier    | -> fee : money[JPY] |
| >=5000JPY          | -       | 0JPY                |
| >=3000JPY <5000JPY | premium | 0JPY                |
| >=3000JPY <5000JPY | regular | 250JPY              |
| <3000JPY           | -       | 500JPY              |

examples
| lines    | tier    | -> fee |
| two      | regular | 0JPY   |
| two      | premium | 0JPY   |
| small    | regular | 500JPY |
| middle   | regular | 250JPY |
| middle   | premium | 0JPY   |
| no_lines | regular | 500JPY |

sequence two
| amount  |
| 3000JPY |
| 2500JPY |

sequence small
| amount  |
| 1200JPY |

sequence middle
| amount  |
| 2000JPY |
| 1500JPY |

sequence no_lines
| amount |
```

**What this one shows**

- **`sum total over lines of amount` leaves one amount behind.** From there it is an ordinary column, so `>=5000JPY` becomes a boundary and completeness and overlap are checked as they always are.
- **The summed column cannot go negative** (E029). The running total then only rises, so the entry guard can refuse the moment it leaves the declared range — which is what keeps the int64 claim true of a sequence whose length nothing caps. For a difference, sum two non-negative columns and subtract.
- **An average does not follow**: dividing by the count is dividing by a variable (E115). Work it out before the call and pass it in.

## Sorting by the head of a part number

The prefix of an SKU decides frozen, chilled or ambient. A `string` column takes a prefix test and nothing else.

```rule
rule sku_prefix_handling v1
description "Decides how a shipment is handled from the prefix of the part number and the number of boxes. An example of matching the start of a string"

enum handling_kind = frozen | chilled | ambient | ambient_mixed

inputs
  sku   : string
  boxes : number  range >=1 <=200

outputs
  handling : handling_kind

# A string cannot be counted, so the only thing a cell can cut on is a prefix. The set of prefixes is finite,
# so completeness and overlap stop exactly as they do for a column of enum values
table handling_of
policy first
| sku                      | boxes | -> handling : handling_kind |
| starts_with "FZ-"        | -     | frozen                      |
| starts_with "CH-", "CL-" | -     | chilled                     |
| -                        | >=20  | ambient                     |
| -                        | <20   | ambient_mixed               |

examples
| sku       | boxes | -> handling   |
| "FZ-1001" | 1     | frozen        |
| "CH-2002" | 5     | chilled       |
| "CL-0003" | 5     | chilled       |
| "AB-0004" | 30    | ambient       |
| "AB-0004" | 3     | ambient_mixed |
```

**What this one shows**

- **A set of prefixes cuts the strings into finitely many classes.** The strings themselves are unbounded, but the partition the column's own cells name is finite, so §6.2's machinery works unchanged — and a witness for a hole comes back as a string you can paste.
- **Comparison is by bytes.** No case folding and no Unicode normalisation: without that, twelve languages would not give one answer.
- **Equality and sets are refused** (E110). Where the values can be enumerated an `enum` fits better. No substring match, no regular expressions.

## Apportioning by ratio

One discount spread over the lines of an order in the ratio of their list prices. Where "one line at a time" above fills each line in turn, this one **splits by ratio**: a line's share is the share up to it minus the share up to the line before.

```rule
rule pro_rata_allocation v1
description "Spreads a lump-sum discount over the lines in proportion to their list prices. One line is one decision, and the caller keeps the running totals. The remainder falls on the last line, so what is handed out always adds up to the total discount"

inputs
  total_off : money[JPY]  range >=0JPY <=1_000_000JPY
  before    : money[JPY]  range >=0JPY <=10_000_000JPY
  upto      : money[JPY]  range >=0JPY <=10_000_000JPY
  base      : money[JPY]  range >=1JPY <=10_000_000JPY
  eligible  : bool

constraint before <= upto
constraint upto <= base

outputs
  share : money[JPY]  round down(1JPY)

derive to_before : money[JPY] = allocate(total_off, before, base)  range >=0JPY <=1_000_000JPY
derive to_upto   : money[JPY] = allocate(total_off, upto, base)    range >=0JPY <=1_000_000JPY

define gap : money[JPY] = to_upto - to_before

table applies
policy unique
| eligible | -> allocated : money[JPY] |
| true     | gap                       |
| false    | 0JPY                      |

result share = allocated

examples
| total_off | before  | upto      | base      | eligible | -> share |
| 1000JPY   | 0JPY    | 3000JPY   | 10_000JPY | true     | 300JPY   |
| 1000JPY   | 3000JPY | 10_000JPY | 10_000JPY | true     | 700JPY   |
| 100JPY    | 0JPY    | 333JPY    | 1000JPY   | true     | 33JPY    |
| 100JPY    | 333JPY  | 666JPY    | 1000JPY   | true     | 33JPY    |
| 100JPY    | 666JPY  | 1000JPY   | 1000JPY   | true     | 34JPY    |
| 0JPY      | 0JPY    | 500JPY    | 1000JPY   | true     | 0JPY     |
| 1000JPY   | 0JPY    | 3000JPY   | 10_000JPY | false    | 0JPY     |
```

**What this one shows**

- **`allocate(<amount>, <running total>, <whole>)` is the one place a rule divides by something that is not a constant.** The answer is `<amount> × <running total> ÷ <whole>` rounded down to a whole unit.
- **Taking the difference makes the parts add up to the amount exactly.** Neighbouring lines add and subtract the same value, so the odd yen lands on the last line. `proofs/` states and proves it (`runTotal_exact`).
- **A `constraint` that the running total stays within the whole is required** (E117). Without it a share can exceed the amount being handed out, and the interval becomes the product of two ranges. Chains count, so it can be written as two lines.

## Inputs taken from the caller's order (JSON Schema)

The shipping fee, decided from an order object. The caller already describes its orders with a JSON Schema, so the rule borrows it with `shape` and says, for each input, where in it the value stands, with `from`. The table itself is a table of flat inputs like any other.

```rule
rule order_shipping_fee v1
description "Decides the shipping fee from an order object. An example of taking inputs out of the caller's contract (§15.125)"

# The caller has already fixed the shape of an order with a JSON Schema. Borrow it, and write where each input comes from in that shape:
# the code that flattens it is generated, and every check holds each path against the contract.
shape order = jsonschema "contracts/order.schema.json" "#/$defs/Order"

enum shipping_zone = honshu | hokkaido | okinawa

inputs
  zone  : shipping_zone  from order.shipping.zone
  cold  : bool  from any order.lines where chilled = true
  lines : number  range >=1 <=50  from count order.lines

outputs
  fee : money[JPY]  round up(10JPY)

# Cold goods cost 500 yen more in any zone, and more than 10 lines cost 200 yen more.
table by_zone
policy unique
| zone     | -> zone_fee : money[JPY] |
| honshu   | 800JPY                   |
| hokkaido | 1200JPY                  |
| okinawa  | 1500JPY                  |

table cold_extra
policy unique
| cold  | -> cold_fee : money[JPY] |
| true  | 500JPY                   |
| false | 0JPY                     |

table bulk_extra
policy unique
| lines | -> bulk_fee : money[JPY] |
| >10   | 200JPY                   |
| <=10  | 0JPY                     |

define total : money[JPY] = zone_fee + cold_fee + bulk_fee

result fee = total

examples
| zone     | cold  | lines | -> fee  |
| honshu   | false | 1     | 800JPY  |
| honshu   | true  | 1     | 1300JPY |
| okinawa  | true  | 11    | 2200JPY |
| hokkaido | false | 10    | 1200JPY |
```

The contract it reads (`contracts/order.schema.json`):

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

**What this one shows**

- **`from` comes in four shapes.** The value of a field (`from order.shipping.zone`), whether some element passes a test (`from any order.lines where chilled = true`), whether every one does (`from all …`), and how many there are (`from count order.lines`). A join or a nested quantifier cannot be written.
- **`rulec gen` also writes `order_shipping_fee_from(order)`.** Hand it the order object as it is, and it reads the inputs out and calls the rule. It is written for Python, TypeScript, JavaScript, Ruby and PHP, where parsed JSON is usually used as it comes, as a plain map; Go, Swift, Java, Rust, SQL, NumPy and Wasm do not get it.
- **Every `rulec check` holds the paths to the contract.** A field the contract does not have is E121, which says how far the path got and which fields were there; a type that does not fit is E120.
- **The contract's validation is held to the inputs' declarations too.** The contract keeps `lines` to 1 to 50 elements (`minItems`, `maxItems`), the same as the `range >=1 <=50` of `lines`. Take `maxItems` away and an order of 51 lines passes the contract but not the rule, which stops at E122; `fix.text` is the keyword to write back into the contract (`"minItems": 1, "maxItems": 50`).
- **No check of the table changes.** What comes out of a projection is a scalar input like any other, and completeness and overlap are decided as they would be without `from`.

## Inputs taken from a Connect request (.proto and Protovalidate)

The shipping fee, decided from a shipment request. The request is described by a `.proto`, and its fields carry Protovalidate's rules. The rule borrows that `.proto`, and its inputs are declared to agree with the rules.

```rule
rule shipment_delivery_fee v1
description "Decides the shipping fee from a shipment request (.proto). An example of keeping the Protovalidate annotations and the input declarations in line (§15.132, §15.133)"

# The caller is a Connect service, and the shape of the request is fixed by a .proto. Borrow it, and write where each input
# comes from in the request. The generated shipment_delivery_fee_from reads the request as it arrives in protojson.
shape shipment = proto "contracts/shipment.proto" shop.v1.CreateShipmentRequest

enum delivery_region = honshu | hokkaido | okinawa
enum delivery_window = morning | evening

inputs
  region   : delivery_region  from shipment.destination.region
  fragile  : bool  from any shipment.parcels where handling = HANDLING_FRAGILE
  parcels  : number  range >=1 <=20  from count shipment.parcels
  declared : money[JPY]  range >=0JPY <=1_000_000JPY  from shipment.declared_value_jpy
  window   : delivery_window?  from shipment.delivery_window

outputs
  fee : money[JPY]  round up(10JPY)

# The fare for one parcel times the number of parcels, plus the extras for fragile goods, insurance and a delivery window.
table by_region
policy unique
| region   | -> per_parcel : money[JPY] |
| honshu   | 800JPY                     |
| hokkaido | 1200JPY                    |
| okinawa  | 1500JPY                    |

table fragile_extra
policy unique
| fragile | -> fragile_fee : money[JPY] |
| true    | 300JPY                      |
| false   | 0JPY                        |

table insurance
policy unique
| declared                | -> insurance_fee : money[JPY] |
| <=30_000JPY             | 0JPY                          |
| >30_000JPY <=100_000JPY | 200JPY                        |
| >100_000JPY             | 500JPY                        |

table window_extra
policy unique
| window           | -> window_fee : money[JPY] |
| none             | 0JPY                       |
| morning, evening | 100JPY                     |

define total : money[JPY] = per_parcel * parcels + fragile_fee + insurance_fee + window_fee

result fee = total

examples
| region   | fragile | parcels | declared     | window  | -> fee    |
| honshu   | false   | 1       | 0JPY         | none    | 800JPY    |
| hokkaido | true    | 2       | 50_000JPY    | none    | 2900JPY   |
| okinawa  | false   | 3       | 200_000JPY   | evening | 5100JPY   |
| honshu   | true    | 20      | 1_000_000JPY | morning | 16_900JPY |
```

The contract it reads (`contracts/shipment.proto`):

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

**What this one shows**

- **The generated `shipment_delivery_fee_from` reads the JSON protojson writes, as it is.** A field is read under its lowerCamelCase name (`declaredValueJpy`) or under the name the `.proto` gives it (`declared_value_jpy`); a field left out is read as its proto default; an int64 that arrives as a string is read as the number it is.
- **An `optional` field is taken by an optional input.** `delivery_window` may not be sent, so the input's type is `delivery_window?`, and the table has a row for when it is missing (`none`).
- **A `where` on an enum field compares the value's name.** `handling` is an enum of the `.proto`, and protojson carries the value's name (`HANDLING_FRAGILE`). At the value numbered 0 (`HANDLING_STANDARD`) the field is left out altogether, and it is read as `HANDLING_STANDARD` all the same.
- **The rules and the declarations agree, so `check` passes.** Take `required = true` off `destination` and a request may leave it out, in which case `region` arrives as `""`. `delivery_region` does not take `""`, so the check stops at E122, with `[(buf.validate.field).required = true]` as `fix.text`.

## Conditions a contract places across its fields (CEL)

The fee, decided from a quote request. The `.proto` the request is validated against promises two things in Protovalidate's CEL: express takes a parcel of up to 5 kg, and the declared value never exceeds the cover. The rule is written on those promises.

```rule
rule express_delivery_quote v1
description "Decides a fare from a quote request (.proto). An example of holding the conditions a contract places across its fields against the rule's constraint and rows (§15.140)"

# The caller's contract promises, in CEL, that an express parcel is at most 5 kg and that the declared value does not exceed the cover.
# The rule writes that promise as a constraint, and the cover table relies on it: it has no row where the declared value exceeds the cover.
shape quote = proto "contracts/quote.proto" shop.v1.QuoteRequest

inputs
  weight   : mass[g]  range >=1g <=30kg  from quote.weight_g
  express  : bool  from quote.express
  declared : money[JPY]  range >=0JPY <=300_000JPY  from quote.declared_jpy
  cover    : money[JPY]  range >=0JPY <=300_000JPY  from quote.cover_jpy

constraint declared <= cover

outputs
  fee : money[JPY]  round up(10JPY)

# Express has two steps, split at 2 kg. The contract lets no express parcel over 5 kg through, so no row is written beyond that.
table by_weight
policy unique
| express | weight      | -> base : money[JPY] |
| false   | <=2kg       | 700JPY               |
| false   | >2kg <=10kg | 1100JPY              |
| false   | >10kg       | 1600JPY              |
| true    | <=2kg       | 1200JPY              |
| true    | >2kg        | 1800JPY              |

table insurance
policy unique
| declared    | cover       | -> cover_charge : money[JPY] |
| <=50_000JPY | <=50_000JPY | 0JPY                         |
| <=50_000JPY | >50_000JPY  | 300JPY                       |
| >50_000JPY  | >50_000JPY  | 600JPY                       |

define total : money[JPY] = base + cover_charge

result fee = total

examples
| weight | express | declared  | cover      | -> fee  |
| 1kg    | false   | 0JPY      | 0JPY       | 700JPY  |
| 3kg    | true    | 20_000JPY | 100_000JPY | 2100JPY |
| 12kg   | false   | 80_000JPY | 100_000JPY | 2200JPY |
| 2kg    | true    | 50_000JPY | 50_000JPY  | 1200JPY |
```

The contract it reads (`contracts/quote.proto`):

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

**What this one shows**

- **`constraint declared <= cover` can be written because the contract promises it.** The insurance table has no row where the declared value exceeds the cover: with the constraint, the completeness check does not ask for one. The contract's CEL (`this.declared_jpy <= this.cover_jpy`) promises the same, so `check` passes. Make the constraint `<` and the contract lets through a request whose declared value equals the cover: the check stops at E123, with that request as the example.
- **There is no row for express above 5 kg.** The contract's `!this.express || this.weight_g <= 5000` never lets such a request through. Add one and it is W124: each cell alone asks for values the contract lets through, and the combination never passes.
- **Of CEL, what can be read is read.** Comparisons of whole-number sums, `in`, `size()`, `has()`, `&&`, `||`, `!` and `? :`. A part that cannot be read, such as a remainder or a string function, is taken as true, so nothing is missed.

## One event in an order that goes on (machine)

Where an online order moves on an event — payment, shipment, delivery, a cancel request — and how much is refunded. The state lives with the caller, in the order's row of a database. The rule stays a pure function that takes the state and one event and answers the next state; the `machine` section says that calling it again and again cannot break.

```rule
rule order_lifecycle v1
description "How an order of an online shop moves between states on the events of payment, shipping, delivery and a cancel request, and how much is refunded. The caller keeps the state, and the rule decides one event only. An example written for the purpose"

enum order_state = received | paid | shipped | delivered | cancelled
enum order_event = pay | ship | deliver | cancel

inputs
  state       : order_state
  event       : order_event
  amount_paid : money[JPY, incl_tax]  range >=0JPY <=1_000_000JPY

outputs
  next_state : order_state
  refund     : money[JPY, incl_tax]  round down(1JPY)
  accepted   : bool

table step
policy unique
| state     | event             | -> next_state | refund      | accepted |
| received  | pay               | paid          | 0JPY        | true     |
| received  | cancel            | cancelled     | 0JPY        | true     |
| received  | ship, deliver     | state         | 0JPY        | false    |
| paid      | ship              | shipped       | 0JPY        | true     |
| paid      | cancel            | cancelled     | amount_paid | true     |
| paid      | pay, deliver      | state         | 0JPY        | false    |
| shipped   | deliver           | delivered     | 0JPY        | true     |
| shipped   | pay, ship, cancel | state         | 0JPY        | false    |  # A cancellation after shipping is received through the return procedure
| delivered | -                 | state         | 0JPY        | false    |
| cancelled | -                 | state         | 0JPY        | false    |  # A payment notice that arrives after the cancellation does not bring the order back

machine order over step
  carry   state -> next_state
  held    amount_paid
  initial received
  final   delivered, cancelled
  never   shipped after cancelled
  once    refund >0JPY

scenario to_delivery
| event   | amount_paid | -> next_state | refund | accepted |
| pay     | 3000JPY     | paid          | 0JPY   | true     |
| ship    | 3000JPY     | shipped       | 0JPY   | true     |
| deliver | 3000JPY     | delivered     | 0JPY   | true     |

scenario late_pay
| event  | amount_paid | -> next_state | refund  | accepted |
| pay    | 3000JPY     | paid          | 0JPY    | true     |
| cancel | 3000JPY     | cancelled     | 3000JPY | true     |
| pay    | 3000JPY     | cancelled     | 0JPY    | false    |

examples
| state    | event  | amount_paid | -> next_state | refund | accepted |
| received | cancel | 0JPY        | cancelled     | 0JPY   | true     |
| shipped  | cancel | 3000JPY     | shipped       | 0JPY   | false    |
```

**What this one shows**

- **`carry state -> next_state` is the one line with new meaning.** It declares that the `next_state` a call answers is passed as `state` to the next one. The table is an ordinary table, and completeness asks for the row about a cancel request that arrives after shipment. `state` in an output cell hands the state back as it was.
- **`held amount_paid` is a promise the caller keeps.** One order passes the same paid amount on every call. As `constraint` says which combinations do not happen, the claims are then about the sequences of calls that keep the promise, and one that changes the amount halfway is never offered as a counterexample.
- **`final`, `never` and `once` are claims about every sequence of calls.** Rewrite the last row so that a payment arriving after cancellation puts the order back to `paid`, and `check` stops with three errors: the ended `cancelled` has a way out (E124), a cancel request, a payment and a shipment reach `shipped` after `cancelled` (E126), and four calls refund twice (E127) — each with the shortest sequence of calls that breaks it. Read one row at a time, every row looks reasonable.
- **A `scenario` is an example that runs for several calls.** It has no column for the carried `state`: the first call starts in `initial`, `received`, and each later one where the call before it ended.
- **The generated code gets the initial state and a test for a final one** (`INITIAL` and `is_final` in Python). The vectors get sequences of calls, and `rulec test` has each language hand the state it answered to its own next call.
- **A revision is compared in terms of the cases in progress.** `rulec diff` between two versions gives the shortest sequence of calls the two answer differently, and the states from which a case can no longer finish. `replay` plays a log's records through the new version one case at a time, a case being the records that share a `tag`.

## A Stripe PaymentIntent's status (machine)

Transcribed from Stripe's documentation: where a PaymentIntent's status goes when it is confirmed, authenticated, captured or canceled, and when a delayed payment settles. A company's API written down as a rule; the seven statuses are Stripe's own.

```rule
rule payment_intent v1
description "Where a Stripe PaymentIntent's status goes when it is confirmed, authenticated, captured or canceled, and when a delayed payment settles. Transcribed from Stripe's documentation"

# Read on 25 September 2026. The comments name the pages as follows:
#   lifecycle  https://docs.stripe.com/payments/paymentintents/lifecycle
#   status     https://docs.stripe.com/payments/payment-intents/verifying-status
#   confirm    https://docs.stripe.com/api/payment_intents/confirm
#   cancel     https://docs.stripe.com/api/payment_intents/cancel
#   capture    https://docs.stripe.com/api/payment_intents/capture
#   hold       https://docs.stripe.com/payments/place-a-hold-on-a-payment-method
#   3ds        https://docs.stripe.com/payments/3d-secure/authentication-flow
#   errors     https://docs.stripe.com/error-codes (payment_intent_unexpected_state)
# A row the pages do not settle says so at its end, with what was assumed.

enum status = requires_payment_method | requires_confirmation | requires_action | processing | requires_capture | succeeded | canceled
enum event = attach | confirm | authenticate | settle | capture | cancel | expire
# The two methods are Stripe's own enums, and both have `automatic` and `manual`. The hold
# page also describes `automatic_delayed`, in private preview and not in the object's enum.
enum capture_method = automatic | automatic_async | manual
enum confirmation_method = automatic | manual
enum funds = untouched | held | captured | released

group unconfirmed = requires_payment_method, requires_confirmation

inputs
  status              : status
  event               : event                # attach: a payment method is attached without confirming
  limit_reached       : bool                 # this confirmation is past the limit Stripe puts on one PaymentIntent, which varies
  needs_action        : bool                 # the payment method asks for a further step, such as 3D Secure
  approved            : bool                 # the issuer or the bank lets the attempt through; for a cancel, Stripe accepts it
  delayed             : bool                 # the payment method confirms success only days later, as a bank debit does
  capture_method      : capture_method
  confirmation_method : confirmation_method

outputs
  next_status : status
  funds       : funds
  refused     : bool                         # the status does not take this event: an API call gets payment_intent_unexpected_state

# The 3ds page tells the step after authentication differently: Stripe attempts the charge and
# the PaymentIntent moves to processing, card or not. The lifecycle and status pages keep
# processing for payment methods that confirm later, and the rows below follow them.
table transition  # lifecycle, status, and the API reference for each call
policy unique
| status           | event                                          | limit_reached | needs_action | approved | delayed | capture_method             | confirmation_method | -> next_status          | funds     | refused |
| unconfirmed      | attach                                         | -             | -            | -        | -       | -                          | -                   | requires_confirmation   | untouched | false   |  # lifecycle; from requires_confirmation, assumed the same
| unconfirmed      | confirm                                        | true          | -            | -        | -       | -                          | -                   | canceled                | untouched | false   |  # confirm
| unconfirmed      | confirm                                        | false         | true         | -        | -       | -                          | -                   | requires_action         | untouched | false   |  # confirm
| unconfirmed      | confirm                                        | false         | false        | false    | -       | -                          | -                   | requires_payment_method | untouched | false   |  # confirm
| unconfirmed      | confirm                                        | false         | false        | true     | -       | manual                     | -                   | requires_capture        | held      | false   |  # confirm
| unconfirmed      | confirm                                        | false         | false        | true     | true    | automatic, automatic_async | -                   | processing              | untouched | false   |  # lifecycle
| unconfirmed      | confirm                                        | false         | false        | true     | false   | automatic, automatic_async | -                   | succeeded               | captured  | false   |  # confirm
| unconfirmed      | cancel                                         | -             | -            | -        | -       | -                          | -                   | canceled                | untouched | false   |  # cancel
| unconfirmed      | authenticate, settle, capture, expire          | -             | -            | -        | -       | -                          | -                   | status                  | untouched | true    |  # capture, errors
| requires_action  | authenticate                                   | -             | -            | false    | -       | -                          | -                   | requires_payment_method | untouched | false   |  # 3ds
| requires_action  | authenticate                                   | -             | -            | true     | -       | -                          | manual              | requires_confirmation   | untouched | false   |  # confirm
| requires_action  | authenticate                                   | -             | -            | true     | -       | manual                     | automatic           | requires_capture        | held      | false   |  # 3ds
| requires_action  | authenticate                                   | -             | -            | true     | true    | automatic, automatic_async | automatic           | processing              | untouched | false   |  # lifecycle
| requires_action  | authenticate                                   | -             | -            | true     | false   | automatic, automatic_async | automatic           | succeeded               | captured  | false   |  # 3ds, lifecycle
| requires_action  | cancel                                         | -             | -            | -        | -       | -                          | -                   | canceled                | untouched | false   |  # cancel
| requires_action  | attach, confirm, settle, capture, expire       | -             | -            | -        | -       | -                          | -                   | status                  | untouched | true    |  # errors; for attach and confirm the pages say nothing, assumed refused
| processing       | settle                                         | -             | -            | true     | -       | -                          | -                   | succeeded               | captured  | false   |  # lifecycle, status
| processing       | settle                                         | -             | -            | false    | -       | -                          | -                   | requires_payment_method | untouched | false   |  # lifecycle
| processing       | cancel                                         | -             | -            | true     | -       | -                          | -                   | canceled                | untouched | false   |  # lifecycle: ACH, ACSS, AU BECS, BACS, NZ BECS and SEPA, inside a window
| processing       | cancel                                         | -             | -            | false    | -       | -                          | -                   | status                  | untouched | true    |  # lifecycle; cancel says "in rare cases"
| processing       | attach, confirm, authenticate, capture, expire | -             | -            | -        | -       | -                          | -                   | status                  | untouched | true    |  # errors
| requires_capture | capture                                        | -             | -            | -        | false   | -                          | -                   | succeeded               | captured  | false   |  # capture, lifecycle
| requires_capture | capture                                        | -             | -            | -        | true    | -                          | -                   | processing              | untouched | false   |  # lifecycle names no method; hold says bank debits cannot be held
| requires_capture | cancel                                         | -             | -            | -        | -       | -                          | -                   | canceled                | released  | false   |  # cancel
| requires_capture | expire                                         | -             | -            | -        | -       | -                          | -                   | canceled                | released  | false   |  # hold, capture
| requires_capture | attach, confirm, authenticate, settle          | -             | -            | -        | -       | -                          | -                   | status                  | untouched | true    |  # errors
| succeeded        | -                                              | -             | -            | -        | -       | -                          | -                   | status                  | untouched | true    |  # lifecycle, status: refunds go through the Refunds API
| canceled         | -                                              | -             | -            | -        | -       | -                          | -                   | status                  | untouched | true    |  # cancel, lifecycle

# A case keeps the two methods it was created with, and may change payment method between
# attempts. Whether a retry uses a card or a bank debit is `delayed`, passed on every call.
machine payment over transition
  carry   status -> next_status
  held    capture_method, confirmation_method
  initial requires_payment_method
  final   succeeded, canceled
  never   succeeded after canceled
  once    funds captured

# 3ds: a card that asks for 3D Secure, and a customer who passes it.
scenario card_with_3ds
| event        | limit_reached | needs_action | approved | delayed | capture_method  | confirmation_method | -> next_status  | funds     | refused |
| confirm      | false         | true         | false    | false   | automatic_async | automatic           | requires_action | untouched | false   |
| authenticate | false         | false        | true     | false   | automatic_async | automatic           | succeeded       | captured  | false   |

# hold: a hotel authorizes at booking and captures at check-out.
scenario authorize_then_capture
| event   | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | -> next_status   | funds    | refused |
| confirm | false         | false        | true     | false   | manual         | automatic           | requires_capture | held     | false   |
| capture | false         | false        | true     | false   | manual         | automatic           | succeeded        | captured | false   |

# hold, capture: nobody captures, and the authorization runs out.
scenario hold_expires
| event   | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | -> next_status   | funds     | refused |
| confirm | false         | false        | true     | false   | manual         | automatic           | requires_capture | held      | false   |
| expire  | false         | false        | false    | false   | manual         | automatic           | canceled         | released  | false   |
| capture | false         | false        | true     | false   | manual         | automatic           | canceled         | untouched | true    |

# lifecycle: a bank debit that fails days later, and a card that goes through on the retry.
scenario debit_fails_card_retries
| event   | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | -> next_status          | funds     | refused |
| confirm | false         | false        | true     | true    | automatic      | automatic           | processing              | untouched | false   |
| cancel  | false         | false        | false    | true    | automatic      | automatic           | processing              | untouched | true    |
| settle  | false         | false        | false    | true    | automatic      | automatic           | requires_payment_method | untouched | false   |
| confirm | false         | false        | true     | false   | automatic      | automatic           | succeeded               | captured  | false   |

# confirm: with manual confirmation, the server confirms again after the customer's step.
scenario manual_confirmation
| event        | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | -> next_status        | funds     | refused |
| attach       | false         | false        | false    | false   | automatic      | manual              | requires_confirmation | untouched | false   |
| confirm      | false         | true         | false    | false   | automatic      | manual              | requires_action       | untouched | false   |
| authenticate | false         | false        | true     | false   | automatic      | manual              | requires_confirmation | untouched | false   |
| confirm      | false         | false        | true     | false   | automatic      | manual              | succeeded             | captured  | false   |

# confirm: declines until Stripe's limit on confirmations is reached.
scenario too_many_confirmations
| event   | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | -> next_status          | funds     | refused |
| confirm | false         | false        | false    | false   | automatic      | automatic           | requires_payment_method | untouched | false   |
| confirm | true          | false        | false    | false   | automatic      | automatic           | canceled                | untouched | false   |
| confirm | false         | false        | true     | false   | automatic      | automatic           | canceled                | untouched | true    |

examples
| status           | event  | limit_reached | needs_action | approved | delayed | capture_method | confirmation_method | -> next_status | funds     | refused |
| requires_capture | cancel | false         | false        | false    | false   | manual         | automatic           | canceled       | released  | false   |
| succeeded        | cancel | false         | false        | false    | false   | automatic      | automatic           | succeeded      | untouched | true    |
| processing       | cancel | false         | false        | true     | true    | automatic      | automatic           | canceled       | untouched | false   |
```

**What this one shows**

- **The combinations the documentation leaves open come out as rows.** Completeness asks for every status and every event. Where no page says — a payment method attached again in `requires_action`, for one — the row is a guess, marked `assumed` at its end. Those are the rows to ask Stripe about.
- **Where two pages read differently, the note above the table says so.** The 3D Secure page has a PaymentIntent move to `processing` after authentication, card or not; the lifecycle page keeps `processing` for payment methods that confirm later. The table follows the second.
- **Two enums have values of the same name.** `capture_method` and `confirmation_method` both have `automatic` and `manual`. A value is read in the enum of the column it is written in.
- **A value may be spelled like a reserved word.** The `held` of `funds` — money an authorization holds — is spelled like the `held` line of the `machine` section. A value never starts a line, so the two are never confused.
- **Four claims, and all of them hold**: a case ends in `succeeded` or `canceled`, never reaches `succeeded` after `canceled`, captures funds at most once, and can finish from every state it reaches. The scenarios are six flows the documentation describes: 3D Secure, an authorization and its capture, an authorization that runs out, a bank debit that fails, and more.

## Whether a return is accepted, in English

A rule with no money in it anywhere, written in English throughout. The answer is one of four words, and every combination of the inputs reaches exactly one row. It is a sketch of a shop's own terms, not a transcription of anyone's.

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

**What this one shows**

- **A rule whose answer is not an amount has the same shape as one whose answer is.** The output is a value of an enum, so no rounding is declared.
- **It is written as `policy unique`.** Keep the rows disjoint and the checker proves that every input reaches exactly one of them. `policy first` would take three rows fewer by letting the refusals at the top swallow the rest — and then which row answered a case would be a question about the order of the rows rather than about the row.
- **`not:` works on a value of an enum, not only on a group.** `not: perishable` says what it says without a `group` being declared for it.

## A parcel tariff in pounds and inches

A tariff written in English: pounds for the weight, inches for the size, USD for the money. A sketch as well — the amounts are made up. It is the one rule in the corpus that reaches the imperial units.

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

**What this one shows**

- **Imperial units are units like any other.** The input is `mass[lb]` and the rows draw on it in `160oz` (16oz to the pound). The unit is part of the type, so `cm` in a column of `in` stops at E103.
- **The tables are stacked.** What the first table produces — `size` — is a column of the second, which is what lets "size from the dimensions" and "price from the size" be two tables rather than one wide one.
- **This is where the rounding declaration earns its keep.** Five percent of a 12USD base is 12.60USD. `up(1USD)` makes that 13USD — and which way it should go is a business decision, which is why the tool will not make it for you.

## A rule written in English — EU air passenger rights

Names and cells are English, so not one ASCII alias appears. The money is EUR and the distance is km. It is a transcription of published law — Article 7 of Regulation (EC) No 261/2004 — whose text is already shaped like a decision table.

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

**What this one shows**

- **An ASCII name needs no alias.** A kanji cannot begin an exported Go identifier, which is why a name written in Japanese carries one (`運賃(fee)` in the Japanese originals of these rules); `distance` does not. Write the rule in English and there are no parentheses anywhere.
- **Two currencies never convert.** `100JPY` in a `money[EUR]` column stops at E103. There is no exchange rate in this tool and there must not be one ([the units](reference.md)).
- **What the text leaves open, the table makes you decide.** Article 7(1)(b) says "between 1500 and 3500 kilometres" and does not say whether either end is included. Since (a) is "1500 kilometres or less", the lower end is open here — and that is a decision, made in the open. Leave it undecided and the checker stops with a gap or an overlap.
- **Sometimes writing the same condition twice is the faithful thing.** The 50% reduction thresholds could be keyed on the band in two columns, but Article 7(2) restates the distance conditions in full. Keying them on distance keeps the rows one-for-one with the text.

## A minimum wage, and the exception that overrides it

The UK hourly minimum wage from 1 April 2026, transcribed from GOV.UK. The page states a rate for each age band and then states the apprentice rate as an exception to it, so this is two tables, the second overriding the first.

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

**What this one shows**

- **A main rule and its exception are two tables, not one wider one.** `overrides by_age` says the apprentice rows take precedence, and the checker puts both through completeness and overlap **as one set** — so the exception cannot leave a hole in the rule it overrides.
- **The document sits beside the rule, pinned.** `source gov = file "…" sha256:…`, and `@gov table1` on each table. From then on an hourly rate that the copy does not show fails (E116).
- **The apprentice sentence is two rows because it is two conditions.** "aged under 19" and "aged 19 or over and in the first year of their apprenticeship" — written as the page writes them rather than folded into one.

## A personal allowance that tapers, and the band above it

The UK Personal Allowance and the Income Tax band an income falls in, transcribed from GOV.UK for 2026-27. The allowance goes down by £1 for every £2 above £100,000 — arithmetic rather than a row, so it is a `derive` that a row names.

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

**What this one shows**

- **A row may hold the name of a computed value.** The middle row of `allowance_of` holds `tapered`, a `derive`; the rows on either side hold the two amounts the page states outright.
- **£1 for every £2 is a multiplication by a rate, and the odd pound is the rounding.** `round down(1GBP)` on the output settles it, and the declaration cannot be left out.
- **A band the page writes as "£12,571 to £50,270" is written here as "above £12,570".** A pound is the unit, so the two are the same set — and the second is the one the checker reads as adjacent to the row above it, with nothing in between.

## The US federal income tax, bracket by bracket

The 2025 tax of an unmarried individual, transcribed from the IRS rate tables (Rev. Proc. 2024-40). Japan's own quick table and this one are the same shape in different clothes: a bracket, a rate, and an amount to start from.

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

**What this one shows**

- **"$X plus Y% of the excess over $Z" is three columns and two lines of arithmetic.** base, rate and floor are transcribed as they stand; `excess` and `tax` are `define`s. Folding them into one deduction — the form Japan's quick table uses — would put a number in the rule that the source does not print.
- **The first bracket carries no citation, deliberately.** The page states it as "10% of the taxable income", with no amount in it, so that row's two zeros are this rule's way of writing that. The citation sits on the rows rather than on the table, and a row's citation says only where that row came from.
- **Cents, not dollars.** `money[USDc]` counts cents, because $1,192.50 is not a whole dollar. The unit is part of the type, so dollars and cents cannot be taken for one another.

## One section of the US Code of Federal Regulations

How far an employee may have to walk to a fire extinguisher, transcribed from 29 CFR 1910.157(d). The section is pinned to a copy the eCFR served for a date — the machinery that holds the Japanese rules to e-Gov, working for a statute written in English.

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

**What this one shows**

- **The database is the word after `law`.** `law ecfr "29 CFR 1910"` reads the eCFR; left out, it is e-Gov. The id is a title and a part, and what a citation adds is one section.
- **A fragment the language cannot read as one word is quoted.** `@osha "§1910.157"`, in the citation and on the pin line alike. The copy lands in `sources/law/29-CFR-1910@2026-01-01/1910.157.xml`, and `rulec source pin` writes its digest.
- **`rulec source outdated` asks the eCFR about that very section.** It answers with the amendment dates and whether each was substantive, so a re-issue that only moved the markup does not send anyone back to the text.
- **The unit is feet**, because the section is: "75 feet (22.9 m)". A length is one integer in its declared unit, and there is no conversion to decide.

## A stamp duty, and the relief that overrides it

Which SDLT band a residential purchase falls in, transcribed from GOV.UK. A main table and a relief for first-time buyers — the same shape as Japan's own stamp duty rule, in another country's words.

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

**What this one shows**

- **The relief runs out.** The page says it cannot be claimed over £500,000, so the relief table has no rows above that and the main rule shows through on its own. That is what `overrides` on a table covering part of the input space means.
- **Only the first row carries no citation.** The page writes that band as "Zero" rather than as a percentage, so `0%` is this rule's way of writing it.
- **The 5% on a second home is a sentence, not a table**, so that table cites the document whole. It is not added to the band here: the tax itself is worked out slice by slice, and the page tabulates no such total.

## How long anyone may be exposed to noise

Table G-16 of 29 CFR 1910.95: a permitted duration for each sound level. The section is pinned to a copy the eCFR served for a date.

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

**What this one shows**

- **How to read a level the table does not list is decided here.** 91 dBA is given the 92 dBA row — the shorter of the two durations. The appendix says the reference duration is computed by a formula, but the formula is a picture in the document and no text of it comes out of the copy; rounding the other way would permit longer exposure than the formula does.
- **Sound is a type with comparison and range and nothing else.** Decibels do not add: two of them summed are not two sounds' worth.
- **The duration is held in minutes**, because the table has 1½ hours and ¼ hour in it. `duration[min]` writes those as 90 and 15 with nothing left over.

## Whether an excavation needs protection from cave-ins

29 CFR 1926.652(a)(1). Another part of the same title, and so a source of its own. The answer is not an amount but a word: required, or not.

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

**What this one shows**

- **There are two exceptions, and one of them is two conditions.** Stable rock throughout; or less than five feet deep **and** an examination by a competent person giving no indication of a potential cave-in. The second takes two columns because the section takes two clauses.
- **"Not examined" is not "examined and found nothing".** So the input is the examination's answer rather than whether one was made, and a site nobody looked at falls into the row that requires the system.
- **It is written in feet**, because the section is: "5 feet (1.52m)".

## What PayPal takes from one payment

The PayPal Checkout fee on a payment in the United States, transcribed from the published merchant fees. Not a statute but a company's own terms — the English counterpart of the Japanese payment-fee rule.

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

**What this one shows**

- **Both the rate and the fixed fee come from the source**: 3.49% and 0.49 USD. The combined rate for an international payment (4.99%) is not printed there, so it is not written here either; the 1.5% is a row of its own.
- **The rounding direction is a placeholder.** The fee has fractions of a cent in it and the page says nothing about them, so the rule declares `half_up` and says in a comment that this is where a decision has to go. A person makes it.
- **It counts in cents.** `money[USDc]` is an integer number of cents, and $0.49 is `49USDc`, so dollars and cents cannot be taken for one another.

## Stamp duty on a receipt

The table for document type 17 (a receipt for the proceeds of a sale) from NTA tax answer No.7141. Besides the amount received, whether an amount is stated and whether the receipt is in the course of business decide it.

```rule
rule receipt_stamp_duty v1
description "The stamp duty on a receipt for money or securities received for the price of goods sold (a Class 17, item 1 document). Receipts under 50,000 yen and those not related to business are exempt"

inputs
  amount   : money[JPY]  range >=0JPY <=1_000_000_000_000JPY
  stated   : bool
  business : bool

outputs
  tax : money[JPY]  round down(1JPY)

table tax_table  # Source: National Tax Agency, Tax Answer No. 7141, List of stamp duty amounts (part 2), Class 17 documents (laws and regulations as of 1 April 2026)
policy unique
| business | stated | amount                             | -> tax : money[JPY] |
| false    | -      | -                                  | 0JPY                |  # Not related to business: exempt
| true     | false  | -                                  | 200JPY              |  # No amount stated on the receipt
| true     | true   | <50_000JPY                         | 0JPY                |  # Exempt
| true     | true   | >=50_000JPY <=1_000_000JPY         | 200JPY              |
| true     | true   | >1_000_000JPY <=2_000_000JPY       | 400JPY              |
| true     | true   | >2_000_000JPY <=3_000_000JPY       | 600JPY              |
| true     | true   | >3_000_000JPY <=5_000_000JPY       | 1000JPY             |
| true     | true   | >5_000_000JPY <=10_000_000JPY      | 2000JPY             |
| true     | true   | >10_000_000JPY <=20_000_000JPY     | 4000JPY             |
| true     | true   | >20_000_000JPY <=30_000_000JPY     | 6000JPY             |
| true     | true   | >30_000_000JPY <=50_000_000JPY     | 10_000JPY           |
| true     | true   | >50_000_000JPY <=100_000_000JPY    | 20_000JPY           |
| true     | true   | >100_000_000JPY <=200_000_000JPY   | 40_000JPY           |
| true     | true   | >200_000_000JPY <=300_000_000JPY   | 60_000JPY           |
| true     | true   | >300_000_000JPY <=500_000_000JPY   | 100_000JPY          |
| true     | true   | >500_000_000JPY <=1_000_000_000JPY | 150_000JPY          |
| true     | true   | >1_000_000_000JPY                  | 200_000JPY          |

examples
| amount       | stated | business | -> tax |
| 49_999JPY    | true   | true     | 0JPY   |  # Under 50,000 yen is exempt
| 50_000JPY    | true   | true     | 200JPY |
| 1_000_000JPY | true   | true     | 200JPY |  # 1,000,000 yen or less
| 1_000_001JPY | true   | true     | 400JPY |  # More than 1,000,000 yen
| 0JPY         | false  | true     | 200JPY |  # No amount stated
| 3_000_000JPY | true   | false    | 0JPY   |  # Not related to business
```

**What this one shows**

- **Exempt is a row of 0 yen.** Under 50,000 yen, and receipts not in the course of business, are exempt, and the table holds that as `0JPY` rows: "not taxed" is an answer of the rule too.
- **A receipt with no amount stated does not look at the amount.** The row with `stated` false has `-` in the amount column and is 200 yen whatever the amount. The checker proves that every combination of the three inputs hits exactly one row.

## The income-tax bracket table

The quick-calculation table of NTA tax answer No.2260. Each bracket of taxable income carries a rate and a deduction, and `taxable × rate − deduction` is the tax; the reconstruction surtax is 2.1% of it.

```rule
rule japan_income_tax v1
description "The quick-calculation table of income tax. It multiplies the taxable income by the rate, subtracts the deduction, and adds the special income tax for reconstruction"

inputs
  taxable : money[JPY]  range >=1000JPY <=10_000_000_000JPY  # The amount after dropping the fraction below 1,000 yen. The table starts at 1,000 yen

outputs
  tax    : money[JPY]  round down(1JPY)  # Taxable income is in units of 1,000 yen and the rates are whole percents, so no fraction arises
  surtax : money[JPY]  round down(1JPY)  # 2.1% of the base income tax amount. The page says nothing about a fraction below one yen, so round down is a placeholder

# The table is written as "from 1,000 yen to 1,949,000 yen" and "from 1,950,000 yen". Taxable income is in units of 1,000 yen, so
# the top of each band is the same as just before the start of the next. Here it is written as "below the start of the next band".
table brackets  # Source: National Tax Agency, Tax Answer No. 2260, Income tax rates (laws and regulations as of 1 April 2026)
policy unique
| taxable                        | -> rate : rate[step 1%] | deduction : money[JPY] |
| <1_950_000JPY                  | 5%                      | 0JPY                   |
| >=1_950_000JPY <3_300_000JPY   | 10%                     | 97_500JPY              |
| >=3_300_000JPY <6_950_000JPY   | 20%                     | 427_500JPY             |
| >=6_950_000JPY <9_000_000JPY   | 23%                     | 636_000JPY             |
| >=9_000_000JPY <18_000_000JPY  | 33%                     | 1_536_000JPY           |
| >=18_000_000JPY <40_000_000JPY | 40%                     | 2_796_000JPY           |
| >=40_000_000JPY                | 45%                     | 4_796_000JPY           |

define tax : money[JPY] = taxable * rate - deduction
define surtax : money[JPY] = tax * 2.1%  # Source: the same page, "2.1 percent of the base income tax amount"

examples
| taxable      | -> tax     | surtax    |
| 7_000_000JPY | 974_000JPY | 20_454JPY |  # The same page's worked example: 7,000,000 yen * 0.23 - 636,000 yen = 974,000 yen
| 1_949_000JPY | 97_450JPY  | 2046JPY   |  # The top of the 5% band. 2,046.45 yen, rounded down (a placeholder)
| 1_950_000JPY | 97_500JPY  | 2047JPY   |  # The bottom of the 10% band. Thanks to the deduction it differs from the row above by only 50 yen
```

**What this one shows**

- **One table produces a rate and an amount at once.** The rate column is `rate[step 1%]`, the deduction column `money[JPY]`, and a `define` multiplies and subtracts. The page's own worked example (7,000,000 × 0.23 − 636,000 = 974,000 yen) is an `examples` row as it stands.
- **Bracket edges are written as "below the start of the next bracket".** The page says "from 1,000 to 1,949,000 yen" and "from 1,950,000 yen"; since taxable income is in units of 1,000 yen those are the same thing, and completeness over all the integers needs the form with no gap.
- **What the page does not say is marked as a placeholder.** How a fraction of a yen in the surtax is settled is not on this page. The rule says `round down` and keeps, in the comment beside the declaration, that the source is silent — which is what `rulec doc` shows the approver.

## Stamp duty on a contract, with a reduced rate that expires

The stamp duty on a contract for the transfer of real estate (document type 1). The standard amounts (No.7140) and the reduced amounts for contracts made up to 31 March 2027 (No.7108) sit in one table, with the date of the contract as an input.

```rule
rule japan_stamp_duty v1
description "The stamp duty on a contract for the transfer of real estate (a Class 1 document). It depends on the contract amount stated and the date it was made, and a contract made by 31 March 2027 has the reduced rate"

inputs
  amount : money[JPY]  range >=0JPY <=1_000_000_000_000JPY
  stated : bool
  made   : date  range >=2014-04-01 <=2030-12-31  # From the start of the reduced rate (1 April 2014)

outputs
  tax : money[JPY]  round down(1JPY)

define reduced : bool = made <= 2027-03-31  # Source: No. 7108. Contracts made from 1 April 2014 to 31 March 2027. The start is the lower end of the input's range

# The reduced rate applies only to a contract amount over 100,000 yen (No. 7108). At 100,000 yen or less the main rate applies whatever the period, and 200 yen where no amount is stated.
table tax_table  # Source: National Tax Agency, Tax Answer No. 7140, List of stamp duty amounts (part 1), Class 1 documents, and No. 7108, Reduced stamp duty on contracts for the transfer of real estate (laws and regulations as of 1 April 2026)
policy unique
| stated | reduced | amount                               | -> tax : money[JPY] |
| false  | -       | -                                    | 200JPY              |  # No contract amount stated
| true   | -       | <10_000JPY                           | 0JPY                |  # Exempt
| true   | -       | >=10_000JPY <=100_000JPY             | 200JPY              |
| true   | true    | >100_000JPY <=500_000JPY             | 200JPY              |  # From here, the amounts after the reduction (No. 7108)
| true   | true    | >500_000JPY <=1_000_000JPY           | 500JPY              |
| true   | true    | >1_000_000JPY <=5_000_000JPY         | 1000JPY             |
| true   | true    | >5_000_000JPY <=10_000_000JPY        | 5000JPY             |
| true   | true    | >10_000_000JPY <=50_000_000JPY       | 10_000JPY           |
| true   | true    | >50_000_000JPY <=100_000_000JPY      | 30_000JPY           |
| true   | true    | >100_000_000JPY <=500_000_000JPY     | 60_000JPY           |
| true   | true    | >500_000_000JPY <=1_000_000_000JPY   | 160_000JPY          |
| true   | true    | >1_000_000_000JPY <=5_000_000_000JPY | 320_000JPY          |
| true   | true    | >5_000_000_000JPY                    | 480_000JPY          |
| true   | false   | >100_000JPY <=500_000JPY             | 400JPY              |  # From here, the amounts of the main rate (No. 7140)
| true   | false   | >500_000JPY <=1_000_000JPY           | 1000JPY             |
| true   | false   | >1_000_000JPY <=5_000_000JPY         | 2000JPY             |
| true   | false   | >5_000_000JPY <=10_000_000JPY        | 10_000JPY           |
| true   | false   | >10_000_000JPY <=50_000_000JPY       | 20_000JPY           |
| true   | false   | >50_000_000JPY <=100_000_000JPY      | 60_000JPY           |
| true   | false   | >100_000_000JPY <=500_000_000JPY     | 100_000JPY          |
| true   | false   | >500_000_000JPY <=1_000_000_000JPY   | 200_000JPY          |
| true   | false   | >1_000_000_000JPY <=5_000_000_000JPY | 400_000JPY          |
| true   | false   | >5_000_000_000JPY                    | 600_000JPY          |

examples
| amount        | stated | made       | -> tax    |
| 30_000_000JPY | true   | 2026-09-16 | 10_000JPY |  # A sales contract of 30,000,000 yen. It is in the reduced period, so 10,000 yen (the main rate would be 20,000 yen)
| 30_000_000JPY | true   | 2027-04-01 | 20_000JPY |  # The day after the reduced period ends
| 100_000JPY    | true   | 2026-09-16 | 200JPY    |  # Exactly 100,000 yen is outside the reduction, and the main rate is 200 yen too
| 5000JPY       | true   | 2026-09-16 | 0JPY      |  # Under 10,000 yen is exempt
| 0JPY          | false  | 2026-09-16 | 200JPY    |  # No contract amount stated
```

**What this one shows**

- **A time-limited exception is a date definition and one column.** `define reduced = made <= 2027-03-31` goes into a column: `true` on the reduced rows, `false` on the standard ones, `-` where the period does not matter. Under `policy unique` every amount on every date is proved to hit exactly one row.
- **What the reduction does not cover, the standard rows take.** The reduction applies only above 100,000 yen, so the exempt row (under 10,000 yen) and the row up to 100,000 yen have `-` in the period column.
- **This rule found a defect in the generator.** A date literal inside a definition was generated as 0 in every language. The reference evaluator read the date, so the disagreement showed up in `rulec test`.

## The employees' pension grade table

The premium table for employees' pension from the Japan Pension Service (fiscal 2026 edition): 32 grades, and a rate that is 18.3% for ordinary insured people but varies by fund for members of a pension fund, so the rate is an input.

```rule
rule pension_insurance_premium v1
description "The employees' pension insurance premium. From the monthly pay it takes the standard monthly remuneration, multiplies by the rate and halves it, and gives the amount deducted from salary and the amount paid in cash"

source agency = file "sources/nenkin/R08ryougaku.xlsx" sha256:462a199ad4f3b69c  # Japan Pension Service, premium table of employees' pension insurance from September 2020 (paid in October), fiscal 2026 edition
  表1 sha256:09a23ce9620373c4

inputs
  monthly : money[JPY]  range >=0JPY <=10_000_000JPY
  rate    : rate[step 0.1%]  range >=0% <=30%  # 18.3% for general members, miners and seamen. Members of an employees' pension fund pay 13.3% to 15.9% depending on the fund, so it is an input

outputs
  std    : money[JPY]  round down(1JPY)
  deduct : money[JPY]  round half_down(1JPY)  # Source: note (1) of the premium table. Deducted from salary, 50 sen or less is rounded down and more than 50 sen up
  cash   : money[JPY]  round half_up(1JPY)    # Source: note (2) of the premium table. Paid in cash, less than 50 sen is rounded down and 50 sen or more up

table grade  @agency 表1  # Copied from grades 1 to 32 of the premium table as they are
policy unique
| monthly                  | -> std : money[JPY] |
| <93_000JPY               | 88_000JPY           |
| >=93_000JPY <101_000JPY  | 98_000JPY           |
| >=101_000JPY <107_000JPY | 104_000JPY          |
| >=107_000JPY <114_000JPY | 110_000JPY          |
| >=114_000JPY <122_000JPY | 118_000JPY          |
| >=122_000JPY <130_000JPY | 126_000JPY          |
| >=130_000JPY <138_000JPY | 134_000JPY          |
| >=138_000JPY <146_000JPY | 142_000JPY          |
| >=146_000JPY <155_000JPY | 150_000JPY          |
| >=155_000JPY <165_000JPY | 160_000JPY          |
| >=165_000JPY <175_000JPY | 170_000JPY          |
| >=175_000JPY <185_000JPY | 180_000JPY          |
| >=185_000JPY <195_000JPY | 190_000JPY          |
| >=195_000JPY <210_000JPY | 200_000JPY          |
| >=210_000JPY <230_000JPY | 220_000JPY          |
| >=230_000JPY <250_000JPY | 240_000JPY          |
| >=250_000JPY <270_000JPY | 260_000JPY          |
| >=270_000JPY <290_000JPY | 280_000JPY          |
| >=290_000JPY <310_000JPY | 300_000JPY          |
| >=310_000JPY <330_000JPY | 320_000JPY          |
| >=330_000JPY <350_000JPY | 340_000JPY          |
| >=350_000JPY <370_000JPY | 360_000JPY          |
| >=370_000JPY <395_000JPY | 380_000JPY          |
| >=395_000JPY <425_000JPY | 410_000JPY          |
| >=425_000JPY <455_000JPY | 440_000JPY          |
| >=455_000JPY <485_000JPY | 470_000JPY          |
| >=485_000JPY <515_000JPY | 500_000JPY          |
| >=515_000JPY <545_000JPY | 530_000JPY          |
| >=545_000JPY <575_000JPY | 560_000JPY          |
| >=575_000JPY <605_000JPY | 590_000JPY          |
| >=605_000JPY <635_000JPY | 620_000JPY          |
| >=635_000JPY             | 650_000JPY          |

define half : money[JPY] = std * rate / 2  # Half of the full amount. The fraction below one yen is settled by the rounding of the two outputs above
define deduct : money[JPY] = half
define cash : money[JPY] = half

examples
| monthly    | rate  | -> std     | deduct    | cash      |
| 90_000JPY  | 18.3% | 88_000JPY  | 8052JPY   | 8052JPY   |  # Grade 1 of the table. The full amount is 16,104.00 yen, half is 8,052.00 yen
| 250_000JPY | 18.3% | 260_000JPY | 23_790JPY | 23_790JPY |  # Grade 17 of the table. 250,000 yen or more and less than 270,000 yen
| 700_000JPY | 18.3% | 650_000JPY | 59_475JPY | 59_475JPY |  # Grade 32 of the table (the top). Half is 59,475.00 yen
```

**What this one shows**

- **The same shape as the health-insurance rule.** Fifty grades become thirty-two and the ceiling is 650,000 yen; the halving and the two ways of settling the sen are unchanged. Rules of one shape transcribe into rules of one shape.
- **Here the two ways agree.** 18.3% of a standard remuneration is always an even number of yen, so the half has no fraction. The rule states both roundings; the `examples` show that at this rate the difference never appears.
- **All 32 printed grades are held to the rule.** The printed halves are transcribed into records (`tests/oracle/`), and a test replays the rule over them and requires every one to agree.
- **And the table is held to the workbook it came from.** The `source` line points at the Japan Pension Service's own `.xlsx` and `@agency 表1` cites its first sheet (`表1` is the sheet's own name); `rulec source fetch` takes that sheet out and keeps it beside the rule, and every `rulec check` then requires **each of the 32 standard remunerations to be a value that copy shows** (E116). Write `470_000JPY` as `480_000JPY` and it fails there alone — on the rounding grid, no gap, no overlap.

## A premium table, with two ways to settle the sen

The Kyokai Kenpo (Japan Health Insurance Association) premium table (Tokyo branch, from March 2026). Monthly pay picks one of 50 grades of standard remuneration, the rate is applied and the amount halved. Fractions of a yen are settled two different ways — one when the premium is deducted from salary, another when it is paid in cash — and transcribing this table is what put `half_down` into the language.

```rule
rule health_insurance_premium v1
description "The health insurance premium of Kyokai Kenpo (the Japan Health Insurance Association). From the monthly pay it takes the standard monthly remuneration, multiplies by the rate and halves it, and gives the amount deducted from salary and the amount paid in cash"

inputs
  monthly   : money[JPY]  range >=0JPY <=10_000_000JPY
  rate      : rate[step 0.01%]  range >=0% <=20%  # Changes with the prefecture and the fiscal year, so it is an input
  care_rate : rate[step 0.01%]  range >=0% <=5%   # The part added for Category 2 insured persons of long-term care insurance (aged 40 to 64)
  care      : bool

outputs
  std    : money[JPY]  round down(1JPY)
  deduct : money[JPY]  round half_down(1JPY)  # Source: note (1) of the premium table. Deducted from salary, 50 sen or less is rounded down and more than 50 sen up
  cash   : money[JPY]  round half_up(1JPY)    # Source: note (2) of the premium table. Paid in cash, less than 50 sen is rounded down and 50 sen or more up

derive both : rate[step 0.01%] = rate + care_rate  range >=0% <=25%

table grade  # Source: Japan Health Insurance Association, premium table of health insurance and employees' pension insurance from March 2026 (paid in April), Tokyo branch
policy unique
| monthly                      | -> std : money[JPY] |
| <63_000JPY                   | 58_000JPY           |
| >=63_000JPY <73_000JPY       | 68_000JPY           |
| >=73_000JPY <83_000JPY       | 78_000JPY           |
| >=83_000JPY <93_000JPY       | 88_000JPY           |
| >=93_000JPY <101_000JPY      | 98_000JPY           |
| >=101_000JPY <107_000JPY     | 104_000JPY          |
| >=107_000JPY <114_000JPY     | 110_000JPY          |
| >=114_000JPY <122_000JPY     | 118_000JPY          |
| >=122_000JPY <130_000JPY     | 126_000JPY          |
| >=130_000JPY <138_000JPY     | 134_000JPY          |
| >=138_000JPY <146_000JPY     | 142_000JPY          |
| >=146_000JPY <155_000JPY     | 150_000JPY          |
| >=155_000JPY <165_000JPY     | 160_000JPY          |
| >=165_000JPY <175_000JPY     | 170_000JPY          |
| >=175_000JPY <185_000JPY     | 180_000JPY          |
| >=185_000JPY <195_000JPY     | 190_000JPY          |
| >=195_000JPY <210_000JPY     | 200_000JPY          |
| >=210_000JPY <230_000JPY     | 220_000JPY          |
| >=230_000JPY <250_000JPY     | 240_000JPY          |
| >=250_000JPY <270_000JPY     | 260_000JPY          |
| >=270_000JPY <290_000JPY     | 280_000JPY          |
| >=290_000JPY <310_000JPY     | 300_000JPY          |
| >=310_000JPY <330_000JPY     | 320_000JPY          |
| >=330_000JPY <350_000JPY     | 340_000JPY          |
| >=350_000JPY <370_000JPY     | 360_000JPY          |
| >=370_000JPY <395_000JPY     | 380_000JPY          |
| >=395_000JPY <425_000JPY     | 410_000JPY          |
| >=425_000JPY <455_000JPY     | 440_000JPY          |
| >=455_000JPY <485_000JPY     | 470_000JPY          |
| >=485_000JPY <515_000JPY     | 500_000JPY          |
| >=515_000JPY <545_000JPY     | 530_000JPY          |
| >=545_000JPY <575_000JPY     | 560_000JPY          |
| >=575_000JPY <605_000JPY     | 590_000JPY          |
| >=605_000JPY <635_000JPY     | 620_000JPY          |
| >=635_000JPY <665_000JPY     | 650_000JPY          |
| >=665_000JPY <695_000JPY     | 680_000JPY          |
| >=695_000JPY <730_000JPY     | 710_000JPY          |
| >=730_000JPY <770_000JPY     | 750_000JPY          |
| >=770_000JPY <810_000JPY     | 790_000JPY          |
| >=810_000JPY <855_000JPY     | 830_000JPY          |
| >=855_000JPY <905_000JPY     | 880_000JPY          |
| >=905_000JPY <955_000JPY     | 930_000JPY          |
| >=955_000JPY <1_005_000JPY   | 980_000JPY          |
| >=1_005_000JPY <1_055_000JPY | 1_030_000JPY        |
| >=1_055_000JPY <1_115_000JPY | 1_090_000JPY        |
| >=1_115_000JPY <1_175_000JPY | 1_150_000JPY        |
| >=1_175_000JPY <1_235_000JPY | 1_210_000JPY        |
| >=1_235_000JPY <1_295_000JPY | 1_270_000JPY        |
| >=1_295_000JPY <1_355_000JPY | 1_330_000JPY        |
| >=1_355_000JPY               | 1_390_000JPY        |

table applied  # For Category 2 insured persons of long-term care insurance, the rate is the health insurance rate plus the care insurance rate
policy unique
| care  | -> applied_rate : rate[step 0.01%] |
| true  | both                               |
| false | rate                               |

define half : money[JPY] = std * applied_rate / 2  # Half of the full amount. The fraction below one yen is settled by the rounding of the two outputs above
define deduct : money[JPY] = half
define cash : money[JPY] = half

examples
| monthly    | rate  | care_rate | care  | -> std     | deduct    | cash      |
| 60_000JPY  | 9.85% | 1.62%     | false | 58_000JPY  | 2856JPY   | 2857JPY   |  # Half is 2,856.5 yen. Deducted from salary it is rounded down, paid in cash it is rounded up
| 134_000JPY | 9.85% | 1.62%     | false | 134_000JPY | 6599JPY   | 6600JPY   |  # Half is 6,599.5 yen
| 134_000JPY | 9.85% | 1.62%     | true  | 134_000JPY | 7685JPY   | 7685JPY   |  # Half is 7,684.9 yen. Both round up
| 300_000JPY | 9.85% | 1.62%     | true  | 300_000JPY | 17_205JPY | 17_205JPY |  # Half is 17,205.0 yen
```

**What this one shows**

- **Two outputs from one halved amount, rounded two ways.** The table's notes say: deducted from salary, half a yen or less is dropped and more than half is carried up; paid in cash, less than half is dropped and half or more is carried up. The second is `half_up`; the first is `half_down`. At the grade whose half is 6,599.5 yen the two outputs differ by one yen.
- **The rates are inputs.** They change by prefecture and by year; baking them into the rule would mean rewriting the table at every revision. A `derive` adds the care-insurance rate to the health-insurance rate, and a table picks which applies by whether the person is a category-2 care insured.
- **Every grade is held to the printed table.** The printed halves are transcribed into records (`tests/oracle/`), and a test replays the rule over all 100 of them and requires every one to agree.

## A main rule and a reduced rate as two tables, held to their sources

The stamp duty rule above, split into the main table (Appendix Table 1 of the Stamp Tax Act) and the reduced-rate table (Article 91 of the Special Taxation Measures Act), with the exemption as a clause. Each table cites its own source, and each source is held to the digest of a copy of the text fetched from e-Gov, the Japanese government's statute database.

```rule
rule japan_stamp_duty_split v1
description "The stamp duty on a contract for the transfer of real estate (a Class 1 document). The table of the reduced rate, in force until 31 March 2027, takes precedence over the table of the main rule"

source stamp_act = law "342AC0000000023" asof 2026-04-01
  別表第一 sha256:0ba69792e960021e
source measures_act = law "332AC0000000026" asof 2026-04-01
  第91条 sha256:85faf53f6f6e8196

inputs
  amount : money[JPY]  range >=0JPY <=1_000_000_000_000JPY
  stated : bool
  made   : date  range >=2014-04-01 <=2030-12-31

outputs
  tax : money[JPY]  round down(1JPY)

define reduced : bool = made <= 2027-03-31  @measures_act 第91条

table base  @stamp_act 別表第一  # The Class 1 documents column
policy unique
         | stated | amount                               | -> tax : money[JPY] |
unstated | false  | -                                    | 200JPY              |
r3       | true   | >=10_000JPY <=100_000JPY             | 200JPY              |
r4       | true   | >100_000JPY <=500_000JPY             | 400JPY              |
r5       | true   | >500_000JPY <=1_000_000JPY           | 1000JPY             |
r6       | true   | >1_000_000JPY <=5_000_000JPY         | 2000JPY             |
r7       | true   | >5_000_000JPY <=10_000_000JPY        | 10_000JPY           |
r8       | true   | >10_000_000JPY <=50_000_000JPY       | 20_000JPY           |
r9       | true   | >50_000_000JPY <=100_000_000JPY      | 60_000JPY           |
r10      | true   | >100_000_000JPY <=500_000_000JPY     | 100_000JPY          |
r11      | true   | >500_000_000JPY <=1_000_000_000JPY   | 200_000JPY          |
r12      | true   | >1_000_000_000JPY <=5_000_000_000JPY | 400_000JPY          |
r13      | true   | >5_000_000_000JPY                    | 600_000JPY          |

clause exempt -> tax  @stamp_act 別表第一  # The column of exempt items for Class 1 documents (those whose stated contract amount is under 10,000 yen)
  when stated true and amount <10_000JPY
  then 0JPY

table reduced_rate  @measures_act 第91条
policy unique
overrides base
| reduced | stated | amount                               | -> tax     |
| true    | true   | >100_000JPY <=500_000JPY             | 200JPY     |
| true    | true   | >500_000JPY <=1_000_000JPY           | 500JPY     |
| true    | true   | >1_000_000JPY <=5_000_000JPY         | 1000JPY    |
| true    | true   | >5_000_000JPY <=10_000_000JPY        | 5000JPY    |
| true    | true   | >10_000_000JPY <=50_000_000JPY       | 10_000JPY  |
| true    | true   | >50_000_000JPY <=100_000_000JPY      | 30_000JPY  |
| true    | true   | >100_000_000JPY <=500_000_000JPY     | 60_000JPY  |
| true    | true   | >500_000_000JPY <=1_000_000_000JPY   | 160_000JPY |
| true    | true   | >1_000_000_000JPY <=5_000_000_000JPY | 320_000JPY |
| true    | true   | >5_000_000_000JPY                    | 480_000JPY |

examples
| amount        | stated | made       | -> tax    |
| 30_000_000JPY | true   | 2026-09-16 | 10_000JPY |
| 30_000_000JPY | true   | 2027-04-01 | 20_000JPY |
| 100_000JPY    | true   | 2026-09-16 | 200JPY    |
| 5000JPY       | true   | 2026-09-16 | 0JPY      |
| 0JPY          | false  | 2026-09-16 | 200JPY    |
```

**What this one shows**

- **`overrides base` makes the exception take precedence over the main rule.** Instead of adding a `reduced` column to one table, the tables follow the documents, and one line says which wins. The checks judge completeness and overlaps over the two together.
- **The exemption is a `clause`.** In the appendix table it sits in the column of exempt documents, not in the table of taxable ones, so it is written as a sentence rather than a row.
- **`source` and `@` hold the rule to its documents.** `rulec source fetch` brings copies of the appendix table and Article 91 from e-Gov, `rulec source pin` writes their digests. When the text changes, the check stops and names the tables that cite that place (E038).
- **Row labels** (`r1` …) are the names the trace reports and the names `overrides base:r3` points at.

## A proviso written as a sentence

A tariff table decides the base fee, and two clauses decide the shipping fee: the main text ("regular") and the proviso that makes a member's order of 3,900 yen or more free. The proviso's conditions do not line up as columns, so it is a `clause`, not a table.

```rule
rule shipping_fee_proviso v1
description "The shipping fee of a regular delivery. A fare table decides the base fare, and a proviso that makes an order of 3,900 yen or more by a member free takes precedence over the main rule (the sketch of DESIGN.md §15.67)"

import std/jp/prefectures

enum size_class = S60 | S80
group remote = Hokkaido, Okinawa

inputs
  dest   : jp_prefecture
  size   : size_class
  member : bool
  total  : money[JPY]  range >=0JPY <=10_000_000JPY

outputs
  fee : money[JPY]  round up(10JPY)

table fee_table  # Source: base fare table (sketch), Appendix Table 1
policy unique
| dest        | size | -> base : money[JPY] |
| remote      | S60  | 1150JPY              |
| remote      | S80  | 1400JPY              |
| not: remote | S60  | 820JPY               |
| not: remote | S80  | 1050JPY              |

clause regular -> fee  # Source: Article 3, paragraph 1 (main text)
  when always
  then base

clause free -> fee  # Source: Article 3, paragraph 2, proviso
  when total >=3900JPY and member true
  then 0JPY
  overrides regular

examples
| dest     | size | member | total     | -> fee  |
| Hokkaido | S60  | true   | 3900JPY   | 0JPY    |
| Hokkaido | S60  | true   | 3899JPY   | 1150JPY |
| Tokyo    | S80  | false  | 10_000JPY | 1050JPY |
```

**What this one shows**

- **A `clause` is a one-row table.** The condition under `when`, the value under `then`; checked, generated and traced like a table, firing as `{"table":"free","row":1}`.
- **`overrides regular` makes the proviso take precedence over the main text.** The approver's page says "clause free takes precedence over clause regular. in all 1 pairs that meet, the rows of clause free lie inside the other's (an exception)".
- **A group without an alias** (`group remote = Hokkaido, Okinawa`) is allowed; the generated identifiers number it.

## The rule the next one applies

The rule applied by the next example. Years of service and the reason for leaving decide the number of months paid, and a clause reducing the allowance on voluntary resignation takes precedence over the main rule. It is a sketch from the design document, not a real statute.

```rule
rule retirement_pay v1
description "The main rule of the retirement allowance. The years of service decide the number of months paid, and the reduction for resigning for one's own reasons takes precedence over the main rule (the sketch of DESIGN.md §15.69)"

enum reason_kind = retirement_age | voluntary | death

inputs
  years    : number  range >=1 <=40
  reason   : reason_kind
  base_pay : money[JPY]  range >=100_000JPY <=1_000_000JPY

outputs
  allowance : money[JPY]  round down(1JPY)

table schedule  # Source: Article 20, paragraph 1 (sketch)
policy unique
      | years    | reason                    | -> months : number |
short | <10      | retirement_age, voluntary | 5                  |
mid   | >=10 <25 | retirement_age, voluntary | 20                 |
long  | >=25     | retirement_age, voluntary | 40                 |
death | -        | death                     | 40                 |

define full : money[JPY] = base_pay * months
define reduced : money[JPY] = full * 80%

clause main -> allowance  # Source: Article 20, paragraph 1 (sketch)
  when always
  then full

clause reduction -> allowance  # Source: Article 20, paragraph 2 (sketch)
  when reason voluntary
  then reduced
  overrides main

examples
| years | reason         | base_pay   | -> allowance  |
| 3     | retirement_age | 300_000JPY | 1_500_000JPY  |
| 3     | voluntary      | 300_000JPY | 1_200_000JPY  |
| 30    | death          | 300_000JPY | 12_000_000JPY |
```

**What this one shows**

- **It is checked and generated on its own.** The rule that applies it writes this file's digest in its heading and is held to it.
- **The `reduction` clause can be left out by the applying rule with `except`** — "Article 20 (excluding paragraph 2) applies".

## Applying another rule with its terms read differently

The retirement allowance rule above, applied to part-time staff: "years of service" is read as "period in office", "reason for leaving" as "how the term ended", and the reduction clause is not applied.

```rule
rule part_time_retirement_pay v1
description "The retirement allowance of part-time staff. It applies the retirement allowance rule with its terms read as the period in office and how the term ended, and does not apply the reduction (the sketch of DESIGN.md §15.69)"

enum end_kind = term_end | resignation

inputs
  tenure      : number  range >=1 <=3
  end_reason  : end_kind
  monthly_pay : money[JPY]  range >=100_000JPY <=500_000JPY

outputs
  part_time_allowance : money[JPY]  round down(1JPY)

apply retirement = "retirement_pay.rule" sha256:b3c4704669a8f8ad  # Source: Article 31 (sketch)
  years = tenure
  reason = end_reason with term_end -> retirement_age, resignation -> voluntary
  base_pay = monthly_pay
  except reduction
  allowance -> part_time_allowance

examples
| tenure | end_reason  | monthly_pay | -> part_time_allowance |
| 3      | term_end    | 300_000JPY  | 1_500_000JPY           |
| 3      | resignation | 300_000JPY  | 1_500_000JPY           |
```

**What this one shows**

- **A substitution is `<input of the applied rule> = <value of this rule>`.** Two enums are matched value by value: `with term_end -> retirement_age, resignation -> voluntary`.
- **The check proves that what is passed stays inside the applied rule's ranges (E043).** The period in office is 1 to 3 years, inside the 1 to 40 of years of service; declared from 0, the check stops with that value as the example.
- **The applied rule's tables are expanded into this rule, checked and generated with it.** The trace reports the original table's name: `{"table":"retirement:schedule","row":1,"label":"short"}`. The rows for ten years of service and more are never reached here; they are not errors, and the approver's page lists them as unused by this apply.
- **When the applied rule changes, E040 stops the check.** `rulec diff` shows how many answers move and by how much; once accepted, `rulec source pin` writes the new digest.

## A temperature and a volume decide the label

A transcription of a food storage standard. A temperature in degrees Celsius, a volume in millilitres, and a column that is allowed to hold "not decided yet", all in one rule.

```rule
rule food_storage_standard v1
description "Decides the label to put on and the measure to take from the storage temperature and the volume of a food (a sketch of the standards under the Food Sanitation Act)"

enum kind   = frozen | chilled | ambient
enum action = ok | cool | discard

inputs
  temp    : temperature[℃]  range >=-30℃ <=40℃
  volume  : volume[mL]  range >=0mL <=5000mL
  label   : kind
  recheck : kind?

outputs
  verdict : action
  text    : string

table temp_of
policy unique
| label   | temp         | -> in_range : bool |
| frozen  | <=-15℃      | true               |
| frozen  | >-15℃       | false              |
| chilled | >=0℃ <=10℃ | true               |
| chilled | <0℃         | false              |
| chilled | >10℃        | false              |
| ambient | -            | true               |

table recheck_of
policy unique
| recheck | volume   | -> small : bool |
| none    | -        | false           |
| frozen  | -        | true            |
| chilled | <=1000mL | true            |
| chilled | >1000mL  | false           |
| ambient | -        | false           |

table action_of
policy first
| in_range | small | temp  | -> verdict : action | text : string                      |
| true     | -     | -     | ok                  | "conforming"                       |
| false    | true  | -     | cool                | "needs cooling, in small portions" |
| false    | -     | >25℃ | discard             | "discard"                          |
| false    | -     | -     | cool                | "needs cooling"                    |

examples
| temp  | volume | label   | recheck | -> verdict | text                               |
| -20℃ | 500mL  | frozen  | none    | ok         | "conforming"                       |
| -10℃ | 500mL  | frozen  | frozen  | cool       | "needs cooling, in small portions" |
| 5℃   | 1000mL | chilled | none    | ok         | "conforming"                       |
| 30℃  | 2000mL | chilled | chilled | discard    | "discard"                          |
| 15℃  | 500mL  | chilled | none    | cool       | "needs cooling"                    |
```

**What this one shows**

- **A temperature is a scale to compare against, and nothing more.** A ℃ has a displaced zero, so it can be neither added nor doubled; comparison and `range` are all there is, and `temp - temp` stops at E048.
- **`kind?` is a column that may hold nothing.** Only the cell `none` accepts it, and it never appears in an expression — null is kept out of arithmetic by making the table branch on it.
- **An output may be a string**, such as the label a person reads. A string cannot be a table's *input* column (E110): a value that decides a branch belongs in an enum.

## Area, noise and overtime decide the measure and the cost

A sketch of Japan's office hygiene rules and its noise-exposure guidance. Three dimensions — area, sound and time — and a declared relation between two inputs.

```rule
rule office_hygiene_standard v1
description "Decides the measure the employer takes and the cost it bears from the floor area and the ceiling height (giving the air volume) together with the noise and the overtime hours (a sketch of the Office Health Standards Regulation and of the prevention of noise injury)"

enum action = ok | improve | stop

inputs
  people   : number  range >=1 <=200
  floor    : area[m2]  range >=1m2 <=2000m2
  used     : area[m2]  range >=1m2 <=2000m2
  noise    : sound[dB]  range >=0dB <=130dB
  overtime : duration[h]  range >=0h <=200h

# The occupied area never exceeds the floor area. The check asks for no row outside that
constraint used <= floor

outputs
  verdict : action
  cost    : money[JPY, incl_tax]  round half_even(100JPY)

derive spare : area[m2] = floor - used  range >=-1999m2 <=1999m2

table space_of
policy first
| spare | -> tight : bool |
| <10m2 | true            |
| -     | false           |

table action_of
policy first
| noise  | overtime | tight | people | -> verdict : action | rate : rate[step 10%] |
| >=90dB | -        | -     | -      | stop                | 100%                  |
| >=85dB | -        | -     | -      | improve             | 50%                   |
| -      | >45h     | -     | -      | improve             | 50%                   |
| -      | -        | true  | >=50   | stop                | 100%                  |
| -      | -        | true  | -      | improve             | 30%                   |
| -      | -        | -     | -      | ok                  | 0%                    |

define cost : money[JPY, incl_tax] = 10_000JPY * rate

examples
| people | floor | used | noise | overtime | -> verdict | cost      |
| 10     | 100m2 | 50m2 | 60dB  | 10h      | ok         | 0JPY      |
| 10     | 100m2 | 95m2 | 60dB  | 10h      | improve    | 3000JPY   |
| 10     | 100m2 | 50m2 | 86dB  | 10h      | improve    | 5000JPY   |
| 10     | 100m2 | 50m2 | 95dB  | 10h      | stop       | 10_000JPY |
| 10     | 100m2 | 50m2 | 60dB  | 60h      | improve    | 5000JPY   |
```

**What this one shows**

- **`constraint used <= floor` is a relation the caller guarantees.** Completeness then demands no row outside it, every witness becomes a case somebody could really send, and the generated code refuses a violating input at the door.
- **An area is a dimension of its own, not the product of two lengths.** `length * width` is E103: this tool does no dimensional analysis, and will not invent a dimension to hold a product.
- **A sound level is another scale to compare against.** It is logarithmic, so adding two decibels is not two sounds' worth.
- **`round half_even` is one of the five roundings**, the one that sends a tie to the even side — the direction accounting usually asks for.
