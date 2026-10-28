## ritsu mod report: `one` → the working tree

| Check | Errors | Warnings |
|---|---|---|
| With the lock of `one` | 0 | 0 |
| With the lock of the working tree | 0 | 0 |

### `example.com/acme/pricing` (`@pricing`) 1.0.0 → 1.1.0: upgraded

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

`@pricing@1.0.0/rules/shipping_fee.rule` → `@pricing@1.1.0/rules/shipping_fee.rule`

- `region.eu` — an enum gained a value

| where they differ | answer | example | cells |
|---|---|---|---|
| region = domestic  and  weight >=2001g <=2500g | fee: 695 → 395 | region=domestic, weight=2001 | 2 |
| region = world  and  weight >=2001g <=2500g | fee: 2950 → 1850 | region=world, weight=2001 | 2 |

Of the inputs both versions accept, those outside this region get the same answer.

</details>

#### Interface

- `rules/shipping_fee.rule` (rulec, api): 11 changes, 1 of them may break a user

| Change | Where | Old → new |
|---|---|---|
| may break: changed | `/connect/enums/region/values/world/number` | `2 → 3` |
| added | `/{python,typescript,javascript,rust,ruby,php,go,swift,java,connect}/enums/region/values/eu` |  |

A change that may break a user is a candidate: whether it does is for that user's check to say.

A difference is shown in behavior or in an interface.

The check with the lock of the working tree has no error; a difference is shown in behavior.
