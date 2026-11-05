### ritsu mod diff: `example.com/acme/fines` 1.0.0 → 1.1.0

- Old: `old`
- New: `new`
- Files: 1 added, 1 removed, 3 changed, 8 moved in reviewed/
- require: no change

| Change | File |
|---|---|
| added | `sources/law/29-CFR-1903@2025-02-01/1903.15.xml` |
| removed | `sources/law/29-CFR-1903@2024-02-01/1903.15.xml` |
| changed | `fine.rule` |
| changed | `fines.req` |
| changed | `ritsu.mod` |

#### Behavior

<details><summary><code>fine.rule</code> (rulec): the answers differ</summary>

**Version diff (over the whole input space)**

`old/fine.rule` → `new/fine.rule`

- `most` — an output changed range (`100..100` → `200..200`)

| where they differ | answer | example | cells |
|---|---|---|---|
| violation = serious | most: 100 → 200 | violation=serious | 1 |
| violation = other | most: 100 → 200 | violation=other | 1 |

Outside this region the two versions answer alike.

</details>

#### Interface

- `fine.rule` (rulec, api): 24 changes, 22 of them may break a user

| Change | Where | Old → new |
|---|---|---|
| may break: changed | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/most/range/min` | `100 → 200` |
| may break: changed | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/most/range/max` | `100 → 200` |
| may break: changed | `/go/output_fields/most/range/min` | `100 → 200` |
| may break: changed | `/go/output_fields/most/range/max` | `100 → 200` |
| changed | `/sources/osha/asof` | `"2024-02-01" → "2025-02-01"` |
| changed | `/sources/osha/pins/0/sha256` | `"8f4ff5b70afaa6f2" → "22b3990a87aafd3d"` |

A change that may break a user is a candidate: whether it does is for that user's check to say.

#### Requirements

- **`amounts`** (`fines.req`): changed
  - Was: For penalties proposed after January 15, 2024, a penalty is at most $100.
  - Now: For penalties proposed after January 15, 2025, a penalty is at most $200.
  - In force: `2024-01-16..` → `2025-01-16..`
  - Decided 2026-01-01 by maintainer: Follows the amendment at 90 FR 1861 (January 10, 2025).
  - From `@osha §1903.15` (`29 CFR 1903`, as of `2024-02-01` → `2025-02-01`): changed
  - Met by `rulec "fine.rule" table amounts`: changed
  - Checked by `rulec "fine.rule"`: changed
  - Looked at 2026-01-02 by maintainer

<details><summary><code>@osha §1903.15</code>: what its text says differently</summary>

```diff
- (d) For penalties proposed after January 15, 2024, a penalty shall not exceed $100.
- [36 FR 17850, as amended at 89 FR 1817, Jan. 11, 2024]
+ (d) For penalties proposed after January 15, 2025, a penalty shall not exceed $200.
+ [36 FR 17850, as amended at 89 FR 1817, Jan. 11, 2024; 90 FR 1861, Jan. 10, 2025]
```

</details>

These are the words of the module's requirements file. ritsu checks that what they were written against has not moved since someone looked, not that they are right.

A difference is shown in behavior or in an interface.
