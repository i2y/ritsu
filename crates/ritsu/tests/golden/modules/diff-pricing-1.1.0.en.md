### ritsu mod diff: `example.com/acme/pricing` 1.0.0 → 1.1.0

- Old: `example.com/acme/pricing@1.0.0`
- New: `example.com/acme/pricing@1.1.0`
- Files: 2 changed, 2 the same
- require: no change

| Change | File |
|---|---|
| changed | `ritsu.mod` |
| changed | `rules/shipping_fee.rule` |

#### Behavior

- `rules/order_total.rule` (rulec): the same answers

<details><summary><code>rules/shipping_fee.rule</code> (rulec): the answers differ</summary>

**Version diff (over the whole input space)**

`example.com/acme/pricing@1.0.0/rules/shipping_fee.rule` → `example.com/acme/pricing@1.1.0/rules/shipping_fee.rule`

- `region.eu` — an enum gained a value

| where they differ | answer | example | cells |
|---|---|---|---|
| region = domestic  and  weight >=2001g <=2500g | fee: 695 → 395 | region=domestic, weight=2001 | 2 |
| region = world  and  weight >=2001g <=2500g | fee: 2950 → 1850 | region=world, weight=2001 | 2 |

Of the inputs both versions accept, those outside this region get the same answer.

</details>

#### Interface

- `rules/shipping_fee.rule` (rulec, api): 10 changes, 0 of them may break a user

| Change | Where | Old → new |
|---|---|---|
| added | `/{python,typescript,javascript,rust,ruby,php,go,swift,java,connect}/enums/region/values/eu` |  |

A difference is shown in behavior or in an interface.
