# Does your rule fit

rulec is for **a rule that decides one transaction, in one shot, from flat facts**, and
hands back an amount, a yes/no, a class or an order. This page is the
test for whether yours is one of them: five questions, and then the two things people ask
about most — apportionment, and what the language is not for. Last come why the language has
this shape, and what else is out there.

## The five questions

If all five are **yes**, it fits. Whether money is involved is not one of them.

1. Are the inputs **flat values** (you can pass the prefecture itself, not
   `order.destination.prefecture`)?
2. Is the **number of them fixed** — or, where it is not, are they **elements of one
   shape**, one after another (`elements` and `fold`; see [Writing a rule](tour.md))?
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
[Write a rule](tour.md#a-main-rule-and-its-exceptions-as-two-tables), and working examples
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

## Why this shape

Because these five hold at once, and every feature of the tool is paid for by one of
them.

| the property | what the tool spends on it |
|---|---|
| **Being wrong costs something** | nothing is approximated to make it pass; what cannot be proved stops. If the cost is money, units and tax flags in the types and mandatory rounding apply; if it is "wrongly refused, or wrongly let through", the gap and overlap checks are what apply |
| **The right answer is written down elsewhere** | a tariff, a set of terms, a contract. The work is not designing something from nothing, it is **transcribing** — which is why the first thing the tool is worth is the gap showing up as you copy it across |
| **Conditions interlock** | destination × size × weight, kind × period × tier. It does not fit in one `if`, and no one can confirm by eye that the combinations are covered |
| **It is revised on a date** | a tariff revision, a campaign window, a change of terms. So you can point at two versions and get, before you deploy, how many records change and by how much |
| **The writer is not the decider** | the amount, the rounding direction and where a class begins are all business decisions. So there is a rendering for the person who approves, and a checker that turns what it cannot decide into a question with a real case in it |

## What the language is trying to be

**One file doing three jobs.** The same `.rule` is the **specification** a person
approves, the **subject** the checker proves things about, and the **source** the
generated code comes from. The moment those become three files, one of them rots — and
it is almost always the specification.

## What else is out there

**Decision tables are not new, and neither is checking them.** Tables proved free of gaps and
overlaps go back to the formal methods of the 1990s, and most rules engines check a table as
it is edited. An honest look at the neighbours — read off their published material, not from
first-hand use.

| | What it is | How rulec differs |
|---|---|---|
| **Tables in formal methods** — SCR (US Naval Research Laboratory), PVS tables, the [Tabular Expression Toolbox](https://www.mathworks.com/matlabcentral/fileexchange/28812-tabular-expression-toolbox) | Requirements written as tables whose coverage and disjointness are proved, with the case that breaks them when they fail ([SCR](http://www.cs.toronto.edu/~chechik/courses99/ece450/1997heitmeyer-compass97.pdf), [PVS](https://pvs.csl.sri.com/doc/tacas97.pdf)) | The two properties are theirs, and so is refusing a table that lacks them. They were aimed at control software and a theorem prover; rulec aims them at tariffs, terms and statutes, with money, units and rounding, and at code in the languages applications are written in |
| **DMN** (the OMG standard) and its implementations — Apache KIE / Drools, Camunda, jDMN, Kogito, Trisotech | The industry standard for decision tables, with hit policies, and static gap/overlap analysis in several implementations ([Drools DMN](https://kie.apache.org/drools/dmn/), [dmn-check](https://github.com/red6/dmn-check)). [jDMN](https://github.com/goldmansachs/jdmn) generates Java | A DMN cell holds a FEEL expression, so completeness is hard in general and the analyses work over a subset. rulec keeps **a cell to its own column** — no cell spans two — which is what puts completeness and overlap on the decidable side. Units and tax class as types, mandatory rounding, an int64 proof, generating into several languages, and comparison against a legacy implementation are all outside DMN |
| **Rules engines** — Drools DRL, IBM ODM, Pega, SAP BRFplus, [GoRules / ZEN](https://github.com/gorules/zen), OpenRules, OpenL Tablets | Evaluate rules at runtime through a library or a service. Most of them also check a decision table for gaps and overlaps as it is edited ([IBM ODM](https://www.ibm.com/docs/en/odm/8.10.0?topic=tables-decision-table-errors-warnings)), and SAP BRFplus can be set to refuse to activate one that fails. IBM ODM can take a column's domain from an XML Schema, so a value added there shows up in the table as a gap | rulec **ships no engine**. What comes out is a dependency-free ordinary function, and rulec is not present at runtime |
| **Corticon** (Progress, commercial) | Rulesheets with a [conflict checker and a completeness checker](https://docs.progress.com/bundle/corticon-js-rule-modeling/page/The-conflict-checker.html). The closest in ambition | Commercial, with its own runtime. rulec hands over plain source and stops there — and carries the comparison side (verify / replay / diff) itself |
| **[LF-ET](https://www.lohrfink.de/en/solutions/lf-et/)** (Lohrfink, commercial) | Checks a decision table for completeness, redundancy and contradiction, and generates code from it in many languages, COBOL and ABAP among them, with no engine at run time | The closest in what comes out. Its conditions are named expressions written in the target language, and the checks are over combinations of their outcomes; a rulec cell is a range or a set on its own column, so the checks reach the values themselves, and units, rounding and int64 come with them |
| **[Catala](https://github.com/CatalaLang/catala)** (Inria) | A language for writing statute law as a program, correctness-first, compiling to several languages. A [proof plugin](https://github.com/CatalaLang/catala/blob/master/compiler/verification/verification.mld) asks Z3 whether a definition can come out empty or two exceptions can apply at once, and returns the case; it warns and does not stop the build | The closest relative in spirit. Different in shape: Catala mirrors the structure of legal text (defaults and exceptions), not decision tables. rulec has exceptions that take precedence over a main rule, provisos and provisions applied to another case too, but its unit stays the table, with gaps and overlaps decided by rectangle arithmetic. Catala computes in arbitrary precision and rounds money to the cent on its own; rulec holds every value to int64 and asks for the rounding, so every language it generates gives the same answer. Unit types and matching a legacy implementation are not Catala's |
| **API contract checks** — [`buf breaking`](https://buf.build/docs/breaking/rules/), [oasdiff](https://www.oasdiff.com/) | Check that a change to a `.proto` or an OpenAPI document is compatible on the wire | They leave the decisions a contract feeds alone, by design: `buf breaking` does not read custom options such as Protovalidate's rules, and to both of them a value added to a request enum is not a breaking change. rulec holds the contract to the rule's inputs (E032, E033, E122), so a change the wire accepts and no row handles fails in CI |
| **[Morphir](https://github.com/finos/morphir)** (FINOS) | Model business logic once in an IR and emit it to many targets | Broad by design; checking a decision table for completeness is not what it is for |

**Each piece exists somewhere already** — gap and overlap proofs since SCR, checking as you
edit in most rules engines, the case that breaks a definition in Catala, checked tables
generated into many languages in LF-ET, the effect of a revision on past records in the
simulations rules engines run. **Two things we have not found anywhere else.** The checks
hand over their evidence: `rulec certificate` prints what the proofs rest on, a
dependency-free Python file and a program built from a Lean development re-check it, and the
Lean development proves that the checks imply the claims — the pattern of a certifying
algorithm, applied to a business table. And the contract an API is called through, with the
ranges, counts, required fields and enum values its validation states, is held to the rule's
inputs in the contract's own CI, so a change is checked for what it does to the decision, not
only to the wire. A rules engine that takes a column's domain from an XML Schema sees an added
enum value as a gap too, but inside the engine. Everything else is the assembly: cells narrow
enough that gaps and overlaps are exactly decidable, a witness every time, units and rounding
in the types, twelve languages with zero dependencies, comparison against what runs today —
and **all of it drivable by an agent through `--format json` alone**, because an agent is the
first user it is built for.

---

[Write a rule (.rule)](tour.md){ .md-button .md-button--primary }
[Examples](examples.md){ .md-button }
