## ritsu impact：`base` → 作業ツリー

答えが動きました（`rule_changes 1`、`allow_gained 1`、`policies_moved 2`）。行き先は `経路 = 持ち主が読む`、`人が読む = true`。持ち主は `経理`。

| 行き先 | 値 |
|---|---|
| `経路` | `持ち主が読む` |
| `人が読む` | `true` |

| 検査 | エラー | 警告 |
|---|---|---|
| `base` | 0 | 0 |
| 作業ツリー | 0 | 0 |

### 変わったファイル

6 個（仕様 2、生成物 0、手書きのコード 0、文書 0、その他 4）。手書きのコードの変わった行 0

<details><summary>一覧</summary>

| 変わり方 | ファイル | 種類 |
|---|---|---|
| 変えた | `refund_band.ja.rule` | 仕様, rulec |
| 変えた | `refunds.ja.req` | 仕様, yuen |
| 消した | `reviewed/065045fbdf8715b9` | その他 |
| 消した | `reviewed/0c9a511a604558c8` | その他 |
| 足した | `reviewed/50d530478e548bef` | その他 |
| 足した | `reviewed/cc1f64aeed4a0e68` | その他 |

</details>

### 振る舞い

<details><summary><code>refund_band.ja.rule</code> (rulec)：答えが変わります</summary>

**版の差分（入力の全体で比べたもの）**

`base/refund_band.ja.rule` → `refund_band.ja.rule`

| 違いが出る入力 | 答え | 例 | 通り |
|---|---|---|---|
| 点数 >=21 <=30 | 区分: 多数 → 少数 | 点数=21 | 2 |

ここに挙げた入力のほかでは、二つの版は同じ答えを返します。

</details>

<details><summary><code>refunds.ja.gate</code> (sekisho)：答えが変わります</summary>

**sekisho diff：ゲート `返金` v1 → v1**

- 古い版：`base/refunds.ja.gate`
- 新しい版：`refunds.ja.gate`

**`返金する`**

両方の版を合わせて 32 通りをたどりました。

| 変わり方 | 通り | 古い版で効いたポリシー | 新しい版で効いたポリシー | 例 |
|---|---|---|---|---|
| 許可が増えた（拒否 → 許可） | 1 | なし | 係は期間内に少数を返金できる | 係を持つ職員、注文、大きさ：少数、期間内：はい（古い版では大きさ：多数） |
| 答えは同じで、効いたポリシーが変わった | 2 | 責任者は期間内に返金できる | 係は期間内に少数を返金できる、責任者は期間内に返金できる | 責任者を持つ職員、注文、大きさ：少数、期間内：はい（古い版では大きさ：多数） |

ほかの組み合わせでは、どの action も同じ決定になります。

</details>

### インターフェース

- `refund_band.ja.rule` のインターフェースは変わりません

### 要件

- **`返金の区分`** (`refunds.ja.req`)：変わった
  - 20 点までの返金は少数、それより多いものは多数
  - 満たすもの `rulec "refund_band.ja.rule" table 区分を決める`
  - 確かめるもの `rulec "refund_band.ja.rule"`：変わった
  - 確認（2027-01-05、開発）

これはモジュールの要件のファイルの言葉です。ritsu が確かめるのは、書かれた相手が確認のあとで動いていないことで、書かれたことが正しいかどうかではありません。

ほかの 3 個のファイルは答えが同じです。

### 要件と持ち主

- `返金の区分 v1` (`refunds.ja.req:7`), 持ち主 `経理`
- `返金できる人 v1` (`refunds.ja.req:16`), 持ち主 `経理`

### 主張

- 分かりません：このプロジェクトに `.geas` はありません

**生成物**：`ritsu.mod` に `target` の行が無いので、確かめていません

<details><summary>事実</summary>

| 名前 | 値 |
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

新しいエラーはなく、決められなかったものもありません。
