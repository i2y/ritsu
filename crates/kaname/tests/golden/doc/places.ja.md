<!-- kaname <version> が tests/modules/places.kn（sha256:5fcd3d0fa9149eb4）から生成したページです。読むためのもので、もとになるのは .kn のほうです。ここを編集しても .kn には戻せません。 -->
# places

- ファイル：`tests/modules/places.kn`（module places v1、sha256:5fcd3d0fa9149eb4）
- 読むファイル：なし
- kaname：<version>

上のファイルと、それが読むファイルを kaname <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

> [!NOTE]
> 関数 9 個: total 5 個、fails range 4 個。規則も koyomi の日付も呼びません。振替は組み立てません。コードに書いた判断はありません。このモジュールは `kaname check` を通りません（エラー 4 個）。関数がどこで失敗しうるかを、このページで示します。

## 表にある判断

このモジュールは、規則も日付のファイルも帳簿も読みません。計算する値は、どれも引数から来ます。

## 関数

関数は、エフェクトで三つに分かれます。total な関数は、宣言した範囲のどの入力にも答えます。実行時に確かめる関数には、検査が示せなかった場所があります。生成したコードはそこで値を確かめ、外れれば、その場所の失敗の種類（`rule` など）で失敗します。止まらないかもしれない関数は、再帰を持ちます。

### total な関数

#### `inner`

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn inner(n: number range >=0 <=5) -> number
  n
```

#### `better`

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn better(a: tier, b: tier) -> tier
  max(a, b)
```

#### `code`

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn code(t: tier) -> string
  match t
    gold => "G"
    silver, basic => "S"
```

#### `or_zero`

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn or_zero(x: number? range >=0 <=9) -> number
  match x
    none => 0
    some v => v + 1
```

#### `share`

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn share(r: rate[step 1%]) -> rate[step 1%]
  r
```

### 実行時に確かめる関数

#### `outer`

実行時に 1 か所を確かめ、`range` で失敗することがあります。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn outer(n: number range >=0 <=9) -> number
  inner(n: n)
```

失敗しうる場所：

- 12 行目（`range`）: `n` が、関数 `inner` の引数 `n` の宣言（range >=0 <=5）の外になることがあります（E301）。たとえば `outer(n: 9)` のとき、9 になります。

#### `boxed`

実行時に 1 か所を確かめ、`range` で失敗することがあります。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn boxed(n: number range >=0 <=9) -> Box
  Box { size: n }
```

失敗しうる場所：

- 15 行目（`range`）: `n` が、Box のフィールド `size` の宣言（range >=0 <=5）の外になることがあります（E301）。たとえば `boxed(n: 9)` のとき、9 になります。

#### `capped`

実行時に 1 か所を確かめ、`range` で失敗することがあります。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn capped(n: number range >=0 <=9) -> number range >=0 <=5
  n
```

失敗しうる場所：

- 18 行目（`range`）: 関数 `capped` の答えが、宣言（range >=0 <=5）の外になることがあります（E301）。たとえば `capped(n: 9)` のとき、9 になります。

#### `root`

実行時に 1 か所を確かめ、`range` で失敗することがあります。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn root(n: number range >=-9 <=9) -> number
  isqrt(n)
```

失敗しうる場所：

- 21 行目（`range`）: `isqrt` に渡す `n` が、負になることがあります（E301）。たとえば `root(n: -9)` のとき、-9 になります。

### 止まらないかもしれない関数

ありません。

## コードにある判断

このモジュールの判断は、どれも表にあります。

## 例

このモジュールには `examples` がありません。
