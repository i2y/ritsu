## ritsu mod report：`two` → `three`

| 検査 | エラー | 警告 |
|---|---|---|
| `two` の lock で | 0 | 0 |
| `three` の lock で | 1 | 0 |

### 新しい診断 (1)

| 種類 | コード | ファイル | 行 | 内容 | モジュール |
|---|---|---|---|---|---|
| error | `dandori E002` | `注文/注文を受ける.flow` | 48 | `送料.outputs` にフィールド `料金` はありません（あるのは 送料） | `example.com/acme/pricing` |

### `example.com/acme/pricing` (`@pricing`) 1.1.0 → 2.0.0：上げた

- 新しい診断：1
- ファイル：変わった 2、同じ 2
- require：変わりません

| 変わり方 | ファイル |
|---|---|
| 変わった | `ritsu.mod` |
| 変わった | `規則/送料.rule` |

#### 振る舞い

- `規則/注文の合計.rule` (rulec)：同じ答えです

<details><summary><code>規則/送料.rule</code> (rulec)：答えが変わります</summary>

**版の差分（入力の全体で比べたもの）**

`@pricing@1.1.0/規則/送料.rule` → `@pricing@2.0.0/規則/送料.rule`

- `料金` — 出力が消えた
- `送料` — 出力が増えた

| 違いが出る入力 | 答え | 例 | 通り |
|---|---|---|---|
| 地域 = 国内  かつ  重さ >=1g <=2500g | 料金: 395GBPc → - | 地域=国内, 重さ=1 | 3 |
| 地域 = EU  かつ  重さ >=1g <=2500g | 料金: 1250GBPc → - | 地域=EU, 重さ=1 | 3 |
| 地域 = その他の地域  かつ  重さ >=1g <=2500g | 料金: 1850GBPc → - | 地域=その他の地域, 重さ=1 | 3 |
| 地域 = 国内  かつ  重さ >=2501g <=30000g | 料金: 695GBPc → - | 地域=国内, 重さ=2501 | 2 |
| 地域 = EU  かつ  重さ >=2501g <=30000g | 料金: 1950GBPc → - | 地域=EU, 重さ=2501 | 2 |
| 地域 = その他の地域  かつ  重さ >=2501g <=30000g | 料金: 2950GBPc → - | 地域=その他の地域, 重さ=2501 | 2 |

両方の版が受け付ける入力のうち、ここに挙げたほかは同じ答えを返します。

</details>

#### インターフェース

- `規則/送料.rule` (rulec、api)：差が 25 件、うち使う側を壊しうるもの 13 件

| 変わり方 | 場所 | 古い値 → 新しい値 |
|---|---|---|
| 消した（壊しうる） | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/料金` |  |
| 消した（壊しうる） | `/go/output_fields/料金` |  |
| 消した（壊しうる） | `/connect/response_fields/料金` |  |
| 変わった（壊しうる） | `/sql/function/signature` | `"…xt) RETURNS TABLE (\"fee\" bigint, \"fee_by_weight_ro…" → "…xt) RETURNS TABLE (\"shipping_fee\" bigint, \"fee_by_…"` |
| 足した | `/{python,typescript,javascript,rust,ruby,php,swift,java,numpy,sql}/outputs/送料` |  |
| 足した | `/go/output_fields/送料` |  |
| 足した | `/connect/response_fields/送料` |  |

壊しうる差は候補です。使う側を壊すかどうかは、使う側の検査が決めます。

振る舞いかインターフェースに違いが見つかりました。

#### 変わる答えを読む手元の行

- `注文/注文を受ける.flow:6` (dandori, `use rule`)：`@pricing/規則/送料.rule`

dandori と kaname には振る舞いの差が無いので、これらのファイルの答えが変わるかは比べていません。

`three` の lock での検査に、エラーが 1 件あります。
