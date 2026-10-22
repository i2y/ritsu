<!-- kaname <version> が handles.ja.kn（sha256:5371def987ac5eea）から生成したページです。読むためのもので、もとになるのは .kn のほうです。ここを編集しても .kn には戻せません。 -->
# ハンドル（handles_ja）

人が入力したハンドルを、サービスが持つ形にする。両端の空白を除き、中の空白の続きをハイフン一つにし、英字を小文字にしてから、ハンドルの決まりに合うかを確かめ、合わない理由を全部言う。テキストから、ハンドルと番号の組（タグ）も読む。handles.kn の日本語の版

- ファイル：`handles.ja.kn`（module ハンドル v1、sha256:5371def987ac5eea）
- 読むファイル：なし
- kaname：<version>

上のファイルと、それが読むファイルを kaname <version> で検査して作ったページです。ファイルのハッシュが今のものと違えば、このページは古くなっています。

> [!NOTE]
> 関数 4 個、total。規則も koyomi の日付も呼びません。振替は組み立てません。コードに書いた判断はありません。

## 表にある判断

このモジュールは、規則も日付のファイルも帳簿も読みません。計算する値は、どれも引数から来ます。

## 関数

関数は、エフェクトで三つに分かれます。total な関数は、宣言した範囲のどの入力にも答えます。実行時に確かめる関数には、検査が示せなかった場所があります。生成したコードはそこで値を確かめ、外れれば、その場所の失敗の種類（`rule` など）で失敗します。止まらないかもしれない関数は、再帰を持ちます。

### total な関数

#### `正規化`（`normalized`）

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn 正規化(normalized)(入力(raw): string) -> string
  入力.words().join("-").lower_ascii()
```

#### `問題`（`problems`）

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn 問題(problems)(ハンドル(handle): string) -> list[string]
  let 長さ = ハンドル.count()
  let 短い = if 長さ < 3 then "3 文字より短い" else none
  let 長い = if 長さ > 20 then "20 文字より長い" else none
  let ほかの文字 = if ハンドル.chars().all(一字 => "abcdefghijklmnopqrstuvwxyz0123456789-".contains(一字)) then none else "a〜z、0〜9、- のほかの文字がある"
  let 始まり = if "abcdefghijklmnopqrstuvwxyz".contains(ハンドル.take(1)) then none else "英字で始まっていない"
  let 予約 = if ["admin", "help", "root", "support"].contains(ハンドル) then "サービスが使う語" else none
  [短い, 長い, ほかの文字, 始まり, 予約].filter_map(x => x)
```

#### `ハンドルにする`（`handle`）

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn ハンドルにする(handle)(入力(raw): string) -> result[string, list[string]]
  let 形 = 正規化(入力: 入力)
  let 理由 = 問題(ハンドル: 形)
  if 理由.count() = 0 then ok(形) else err(理由)
```

#### `タグを読む`（`tag`）

total な関数です。宣言した範囲のどの入力にも答えます。見出しにエフェクトは書いてありません。kaname が推論したものです。

```kn
fn タグを読む(tag)(テキスト(text): string) -> result[タグ, list[string]]
  match テキスト.index_of("#")
    none => err(["ハンドルと番号のあいだに # がない"])
    some 位置 =>
      let ハンドル = try ハンドルにする(入力: テキスト.take(位置))
      match テキスト.drop(位置 + 1).parse_number()
        none => err(["番号が数字で書かれていない"])
        some 番号 => if 番号 >= 1 and 番号 <= 9999 then ok(タグ { ハンドル: ハンドル, 番号: 番号 }) else err(["番号が 1 から 9999 のあいだにない"])
```

### 実行時に確かめる関数

ありません。

### 止まらないかもしれない関数

ありません。

## コードにある判断

このモジュールの判断は、どれも表にあります。

## 例

`kaname check` は、`examples` の行ごとに、ホストが呼ぶときと同じように関数を参照インタプリタで流し、書いてある答えと比べます。参照インタプリタの答えは次のとおりです。

```kn
正規化(入力: "  Ada   Lovelace ")
  = "ada-lovelace"
ハンドルにする(入力: " Grace Hopper")
  = ok("grace-hopper")
ハンドルにする(入力: "9")
  = err(["3 文字より短い", "英字で始まっていない"])
ハンドルにする(入力: "Root")
  = err(["サービスが使う語"])
タグを読む(テキスト: "Ada Lovelace#42")
  = ok(タグ { ハンドル: "ada-lovelace", 番号: 42 })
タグを読む(テキスト: "ada#0")
  = err(["番号が 1 から 9999 のあいだにない"])
```
