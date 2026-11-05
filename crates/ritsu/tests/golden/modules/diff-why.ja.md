### ritsu mod diff：`example.com/acme/fines` 1.0.0 → 1.1.0

- 古い版：`old`
- 新しい版：`new`
- ファイル：足した 1、消した 1、変わった 3、reviewed/ の入れ替わり 8
- require：変わりません

| 変わり方 | ファイル |
|---|---|
| 足した | `sources/law/29-CFR-1903@2025-02-01/1903.15.xml` |
| 消した | `sources/law/29-CFR-1903@2024-02-01/1903.15.xml` |
| 変わった | `fine.rule` |
| 変わった | `fines.req` |
| 変わった | `ritsu.mod` |

#### 振る舞い

<details><summary><code>fine.rule</code> (rulec)：答えが変わります</summary>

**版の差分（入力の全体で比べたもの）**

`old/fine.rule` → `new/fine.rule`

- `most` — 出力の範囲が変わった (`100..100` → `200..200`)

| 違いが出る入力 | 答え | 例 | 通り |
|---|---|---|---|
| violation = serious | most: 100 → 200 | violation=serious | 1 |
| violation = other | most: 100 → 200 | violation=other | 1 |

ここに挙げた入力のほかでは、二つの版は同じ答えを返します。

</details>

#### インターフェース

- `fine.rule` (rulec、api)：差が 24 件、うち使う側を壊しうるもの 22 件

| 変わり方 | 場所 | 古い値 → 新しい値 |
|---|---|---|
| 変わった（壊しうる） | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/most/range/min` | `100 → 200` |
| 変わった（壊しうる） | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/most/range/max` | `100 → 200` |
| 変わった（壊しうる） | `/go/output_fields/most/range/min` | `100 → 200` |
| 変わった（壊しうる） | `/go/output_fields/most/range/max` | `100 → 200` |
| 変わった | `/sources/osha/asof` | `"2024-02-01" → "2025-02-01"` |
| 変わった | `/sources/osha/pins/0/sha256` | `"8f4ff5b70afaa6f2" → "22b3990a87aafd3d"` |

壊しうる差は候補です。使う側を壊すかどうかは、使う側の検査が決めます。

#### 要件

- **`amounts`** (`fines.req`)：変わった
  - 前：For penalties proposed after January 15, 2024, a penalty is at most $100.
  - 今：For penalties proposed after January 15, 2025, a penalty is at most $200.
  - 効く期間：`2024-01-16..` → `2025-01-16..`
  - 決めたこと（2026-01-01、maintainer）：Follows the amendment at 90 FR 1861 (January 10, 2025).
  - 出どころ `@osha §1903.15` (`29 CFR 1903`、`2024-02-01` → `2025-02-01` 時点)：変わった
  - 満たすもの `rulec "fine.rule" table amounts`：変わった
  - 確かめるもの `rulec "fine.rule"`：変わった
  - 確認（2026-01-02、maintainer）

<details><summary><code>@osha §1903.15</code>：本文の違い</summary>

```diff
- (d) For penalties proposed after January 15, 2024, a penalty shall not exceed $100.
- [36 FR 17850, as amended at 89 FR 1817, Jan. 11, 2024]
+ (d) For penalties proposed after January 15, 2025, a penalty shall not exceed $200.
+ [36 FR 17850, as amended at 89 FR 1817, Jan. 11, 2024; 90 FR 1861, Jan. 10, 2025]
```

</details>

これはモジュールの要件のファイルの言葉です。ritsu が確かめるのは、書かれた相手が確認のあとで動いていないことで、書かれたことが正しいかどうかではありません。

振る舞いかインターフェースに違いが見つかりました。
