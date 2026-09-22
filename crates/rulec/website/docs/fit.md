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

## Money does not have to be involved

Here is a rule with no money in it anywhere. Four flat facts go in, one of four words
comes out, and it passes the checks as written.

```rule
rule return_eligibility v1

enum category = electronics | clothing | perishable
enum verdict = accepted | outside_window | condition_failed | not_returnable

inputs
  item    : category
  days    : number  range >=0 <=365
  opened  : bool
  receipt : bool

outputs
  answer : verdict

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
```

What decides it is the **shape of the decision**, not what the values happen to be. So
this is not "a tool for shipping fees" and not "a tool for e-commerce".

## A statute is written the same way

Statutes are full of table-shaped provisions. A tax table such as Appendix Table 1 of the
Stamp Tax Act is a table as it stands, and a reduced rate, a proviso or a provision applied
to another case sits on top of it. Each of those has its own way of being written.

| In the statute | In the rule |
|---|---|
| **A main rule and an exception that takes precedence** (a stamp duty table and the relief for first-time buyers) | two tables, with one line on the exception: `overrides standard` |
| **A proviso**, one line whose conditions do not line up as columns | not a table but a sentence: `clause` |
| **A provision applied to another case** ("Article 20 applies, reading 'years of service' as 'period in office'") | `apply`, with the substitution written as it stands |
| **Which document, and where in it, it was transcribed from** | a `source` line declares the document and `@osha "§1910.157"` — or `@gov table1` for a tariff sheet or a company rule — at the end of a line cites it. The document is a copy of the statute text fetched from e-Gov, the Japanese government's statute database, or, for a policy or a tariff, a file beside the rule. The rule is held to the copy's digest, so a copy that changed stops the check and names the tables citing it; cite a table and **an amount that is not in that copy fails too** |

The checks judge completeness and overlaps over the main rule and its exceptions together,
and for an applied rule they prove that what this rule passes stays inside the applied
rule's ranges. The approver's page quotes the cited text. How to write them is in
[Write a table](tour.md#a-main-rule-and-its-exceptions-as-two-tables), and working examples
are in [Examples](examples.md#a-main-rule-and-a-reduced-rate-as-two-tables-held-to-their-sources).


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
