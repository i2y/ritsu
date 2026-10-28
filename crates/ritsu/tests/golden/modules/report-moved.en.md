## ritsu mod report: `two` → the working tree

> [!WARNING]
> **A tag moved**: `example.com/acme/pricing` 1.1.0 in the lock of `two` is not the one in the lock of the working tree (its digest went from `sha256:<digest>` to `sha256:<digest>`): the same version was published again with other contents, and is not the one checked before.

| Check | Errors | Warnings |
|---|---|---|
| With the lock of `two` | 0 | 0 |
| With the lock of the working tree | 0 | 0 |

### `example.com/acme/pricing` (`@pricing`) 1.1.0 → 1.1.0: the tag moved

- Files: 1 changed, 3 the same
- require: no change

| Change | File |
|---|---|
| changed | `rules/shipping_fee.rule` |

#### Behavior

- `rules/order_total.rule` (rulec): the same answers

<details><summary><code>rules/shipping_fee.rule</code> (rulec): the answers differ</summary>

**Version diff (over the whole input space)**

`@pricing@1.1.0/rules/shipping_fee.rule (sha256:<head>)` → `@pricing@1.1.0/rules/shipping_fee.rule (sha256:<head>)`

| where they differ | answer | example | cells |
|---|---|---|---|
| region = world  and  weight >=2501g <=30000g | fee: 2950 → 3150 | region=world, weight=2501 | 2 |

Outside this region the two versions answer alike.

</details>

#### Interface

- `rules/shipping_fee.rule` (rulec, api): 11 changes, 11 of them may break a user

| Change | Where | Old → new |
|---|---|---|
| may break: changed | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/fee/range/max` | `2950 → 3150` |
| may break: changed | `/go/output_fields/fee/range/max` | `2950 → 3150` |

A change that may break a user is a candidate: whether it does is for that user's check to say.

A difference is shown in behavior or in an interface.

The check with the lock of the working tree has no error; a difference is shown in behavior.
