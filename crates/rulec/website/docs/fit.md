# Does your rule fit

rulec is for **a rule that decides one transaction, in one shot, from a fixed number of
flat facts**, and hands back an amount, a yes/no, a class or an order. This page is the
test for whether yours is one of them: five questions, and then the two things people ask
about most — apportionment, and what the language is not for.

## The five questions

If all five are **yes**, it fits. Whether money is involved is not one of them.

1. Is the **number of inputs fixed** (not a list of variable length)?
2. Are the inputs **flat values** (you can pass the prefecture itself, not
   `order.destination.prefecture`)?
3. Is it **one decision** (iteration and ordering can live in the caller)?
4. Does the **same input always give the same answer** ("today" and the stock level are
   arguments too)?
5. Does **a person approve** the answer, or is the rule **revised on a date**?

If any of 1–4 is no, it does not fit structurally. If only 5 is no, it will work, but
the tool is more than you need.

## Apportionment depends on how you apportion

Spreading a discount across the lines of an order can or cannot be written, depending on
how the split is decided.

**It can — when you fill each line in turn.** "Apply the discount to the lines in order,
up to each line's own value." Make the rule decide one line, and leave the loop to the
caller.

```rule
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
```

The caller walks the lines and subtracts from `残り値引`. Done this way **the total
always comes out exact** — nothing is over- or under-allocated (checked over 2,000
different sets of lines).

**It cannot — when you split by ratio.** "Apportion by each line's share of the list
price" needs `line ÷ total`, and **division is only allowed by a constant**:
`注文金額 ÷ 100円` is fine, `明細 ÷ 合計` is not — dividing by a variable stops at E115. You can work around it by computing
the ratio in the caller and passing it in as a rate — a rate step goes as fine as you
declare it, `rate[step 0.1%]` and beyond — but then whether that ratio is right is no
longer something this tool says anything about.

## What it is not for

- **Workflows** — several steps, carrying state
- **Judgements about a collection itself** — "any line is refrigerated", "three or more
  items in the cart". Flatten those at the boundary and pass the scalar in
- **Branching on a string** — `string` cannot be a table column (E110). A value that
  decides a branch belongs in an `enum`, where the closed set makes the completeness check
  work. No prefix match and no regular expressions either
- **Deciding the weights or the thresholds themselves** — that is optimisation and
  machine learning. **Adding up scores with weights that are already agreed and turning
  the total into a rank is writable** — see "評価ランク" in the [examples](examples.md),
  where no money appears anywhere. What can be proved is which row fires, never whether a
  weight is the right one

---

[Write a table (.rule)](tour.md){ .md-button .md-button--primary }
[Examples](examples.md){ .md-button }
