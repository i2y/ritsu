# Does your rule fit

rulec is for **a rule that decides one transaction, in one shot, from flat facts**, and
hands back an amount, a yes/no, a class or an order. This page is the
test for whether yours is one of them: five questions, and then the two things people ask
about most — apportionment, and what the language is not for.

## The five questions

If all five are **yes**, it fits. Whether money is involved is not one of them.

1. Are the inputs **flat values** (you can pass the prefecture itself, not
   `order.destination.prefecture`)?
2. Is the **number of them fixed** — or, where it is not, are they **elements of one
   shape**, one after another (`elements` and `fold`; see [Writing a table](tour.md))?
3. Is it **one decision** (ordering, and any repetition across decisions, can live in the
   caller)?
4. Does the **same input always give the same answer** ("today" and the stock level are
   arguments too)?
5. Does **a person approve** the answer, or is the rule **revised on a date**?

If any of 1–4 is no, it does not fit structurally. If only 5 is no, it will work, but
the tool is more than you need.

## Apportionment, either way you apportion

Spreading a discount across the lines of an order can or cannot be written, depending on
how the split is decided.

**Filling each line in turn.** "Apply the discount to the lines in order,
up to each line's own value." Make the rule decide one line, and leave the loop to the
caller.

```rule
inputs
  list_price : money[USD, incl_tax]  range >=0USD <=10000USD
  remaining  : money[USD, incl_tax]  range >=0USD <=10000USD
  eligible   : bool

outputs
  applied : money[USD, incl_tax]  round down(1USDc)

define cap : money[USD, incl_tax] = min(list_price, remaining)

table applies
policy unique
| eligible | -> on : money[USD, incl_tax] |
| true     | cap                          |
| false    | 0USD                         |

result applied = on
```

The caller walks the lines and subtracts from `remaining`. Done this way **the total
always comes out exact** — nothing is over- or under-allocated (checked over 2,000
different sets of lines).

**Splitting by ratio.** "Apportion by each line's share of the list price" is written with
`allocate`. Division by a variable is still refused everywhere else (E115); a share is the
one exception.

```rule
constraint price_upto <= price_total

derive share_upto : money[USD] = allocate(discount_total, price_upto, price_total)  range >=0USD <=10000USD

result share = share_upto - share_before
```

All the caller carries is the running total of list prices. A line gets the share up to it
minus the share up to the line before, so the odd yen lands on the last line and **the parts
add up to the amount exactly**. For the fill-in-turn form above that was measured over 2,000
sets of lines; here it is a theorem in `proofs/`.

## What it is not for

- **Workflows** — several steps, carrying state
- **An average over a collection** — it divides by how many there are, which is dividing by
  a variable; compute it before the call and pass it in. **The total and the count are
  writable** (`sum`: "the lines total more than 10,000 yen" becomes a row of a table;
  `count`: "three or more refrigerated items"), and so is picking **one element** out of a
  sequence (`fold`: "refuse if any line is refrigerated", "take the dearest row") 
- **Branching on a whole string** — a `string` column takes a prefix and nothing else
  (`starts_with "CH-"`; anything else is E110), which is enough to sort SKUs or categories
  by their heads. Where the values can be enumerated an `enum` is better: the closed set is
  what lets the checker ask whether every value has a row. No substring match, no regular
  expressions
- **Deciding an input itself** — "is this ticket billing or technical", "is this damage
  minor". Turning a messy state into a value is a person's work, or a model's, not this
  tool's. Hand the value in **as an argument**: the rule stays a pure function, and replay
  and diff keep meaning what they meant. **Whether to act on a confidence** is itself a
  business decision, so it can be a table — a column of `rate[step 0.1%]` and the gaps and
  overlaps in your thresholds come back as findings. Who decided the value can be kept in
  the record ([`by`](formats.md))
- **Deciding the weights or the thresholds themselves** — that is optimisation and
  machine learning. **Adding up scores with weights that are already agreed and turning
  the total into a rank is writable** — see "評価ランク" in the [examples](examples.md),
  where no money appears anywhere. What can be proved is which row fires, never whether a
  weight is the right one

---

[Write a table (.rule)](tour.md){ .md-button .md-button--primary }
[Examples](examples.md){ .md-button }
