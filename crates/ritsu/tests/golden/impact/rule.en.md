## ritsu impact: `base` → the working tree

The answers moved (`rule_changes 1`, `allow_gained 1`, `policies_moved 2`); route: `route = owner_reads`, `person_reads = true`; owners: `finance`.

| Route | Value |
|---|---|
| `route` | `owner_reads` |
| `person_reads` | `true` |

| Check | Errors | Warnings |
|---|---|---|
| `base` | 0 | 0 |
| The working tree | 0 | 0 |

### Files

6 changed (2 spec, 0 generated, 0 code, 0 doc, 4 other); 0 changed lines of code written by hand

<details><summary>The list</summary>

| Change | File | Kind |
|---|---|---|
| changed | `refund_band.rule` | spec, rulec |
| changed | `refunds.req` | spec, yuen |
| removed | `reviewed/04ca290e6dee7f05` | other |
| added | `reviewed/a0f09fd2f2de810c` | other |
| added | `reviewed/bd1b5f808ea30970` | other |
| removed | `reviewed/d505d62dbace6e27` | other |

</details>

### Behavior

<details><summary><code>refund_band.rule</code> (rulec): the answers differ</summary>

**Version diff (over the whole input space)**

`base/refund_band.rule` → `refund_band.rule`

| where they differ | answer | example | cells |
|---|---|---|---|
| items >=21 <=30 | band: large → small | items=21 | 2 |

Outside this region the two versions answer alike.

</details>

<details><summary><code>refunds.gate</code> (sekisho): the answers differ</summary>

**sekisho diff: gate `refunds` v1 → v1**

- Old: `base/refunds.gate`
- New: `refunds.gate`

**`refund_order`**

32 combinations walked over both versions.

| Change | Combinations | Old deciding policies | New deciding policies | Example |
|---|---|---|---|---|
| allow gained (deny → allow) | 1 | none | clerks_refund_small_amounts | User holding clerk, Order, size: small, in_period: yes (in the old version: size: large) |
| the same answer, decided by other policies | 2 | managers_refund_large_amounts | clerks_refund_small_amounts, managers_refund_large_amounts | User holding manager, Order, size: small, in_period: yes (in the old version: size: large) |

For every other combination, every action is decided the same.

</details>

### Interface

- The interface of `refund_band.rule` is the same

### Requirements

- **`bands_of_a_refund`** (`refunds.req`): changed
  - A refund of up to 20 items is small, and a larger one is large
  - Met by `rulec "refund_band.rule" table band`
  - Checked by `rulec "refund_band.rule"`: changed
  - Looked at 2027-01-05 by development

These are the words of the module's requirements file. ritsu checks that what they were written against has not moved since someone looked, not that they are right.

3 other files answer the same.

### Requirements and owners

- `bands_of_a_refund v1` (`refunds.req:7`), owner `finance`
- `who_refunds v1` (`refunds.req:16`), owner `finance`

### Claims

- Not known: this project has no `.geas`

**Generated files**: not checked: `ritsu.mod` has no `target` line

<details><summary>Facts</summary>

| Name | Value |
|---|---|
| `spec_files` | 2 |
| `generated_files` | 0 |
| `code_files` | 0 |
| `doc_files` | 0 |
| `other_files` | 4 |
| `code_lines` | 0 |
| `new_errors` | 0 |
| `new_warnings` | 0 |
| `rule_changes` | 1 |
| `date_changes` | 0 |
| `allow_gained` | 1 |
| `allow_lost` | 0 |
| `policies_moved` | 2 |
| `book_changes` | 0 |
| `interface_changes` | 0 |
| `interface_breaks` | 0 |
| `undecided` | 0 |
| `opaque_specs` | 0 |
| `requirements_moved` | 2 |
| `requirements_unknown` | 0 |
| `claims_touched` | 0 |
| `unclaimed_lines` | 0 |
| `claims_known` | false |
| `generated_stale` | false |
| `generated_checked` | false |
| `generated_unchecked` | 0 |

</details>

No new error, and nothing was left undecided.
