# ライブラリとして公開する

`ritsu gen` は、モジュールを言語ごとのパッケージにして書きます。このパッケージは、アプリケーションがそのまま動かすためのものです。モジュールのコードと、モジュールが require するすべてのモジュールのコード（依存のものは `deps/<名前>/` の下）を持ち、ランタイムは ritsu が試した版に固定します。`ritsu gen --library` は、各パッケージをライブラリとして、それぞれのレジストリが受け取る形で書きます。npm のパッケージ、Python のディストリビューション、Go のモジュールです。ライブラリが持つのは、自分のモジュールのコードです。依存のモジュールにあるものは、その依存のライブラリから名前で import し、マニフェストでは、ライブラリとランタイムを範囲で求めます。使う側は、自分で生成しなくても、モジュールの規則、日付、帳簿、ワークフローを自分のコードから呼べます。`--library` を付けなければ、`ritsu gen` はこれまでと同じものを書きます。

| | `ritsu gen` | `ritsu gen --library` |
|---|---|---|
| 使い道 | アプリケーション。そのまま動かす | ほかのコード。レジストリを通して使う |
| 依存のコード | 中に持つ（`deps/<名前>/`） | 依存のライブラリにあり、名前で import する |
| ランタイムと依存の版 | 試した版に固定 | 試した版からの範囲 |
| npm のパッケージ | `"private": true` | 公開する。`dist/` に組んで出す |

このページの例は、ritsu のテストの三つのモジュール [crates/ritsu/tests/projects/modules](https://github.com/i2y/ritsu/tree/main/crates/ritsu/tests/projects/modules) です。`tax` は VAT の税率を、`pricing` は送料と注文の合計を、`shop` は注文のフローとかごを持ちます。`pricing` の注文の合計は `tax` の税率を当て、`shop` は `pricing` で値段を付けます。

## ライブラリの名前を書く

ライブラリの名前は、言語ごとに、ritsu.mod の `library` の行で決めます。

<!-- pricing/ritsu.mod -->
```text
# The prices of the Acme shop: shipping fees, and order totals with the VAT rate of tax.
module example.com/acme/pricing
version 1.1.0
description "Shipping fees and order totals of the Acme shop"
license "MIT OR Apache-2.0"
source "https://github.com/acme/pricing"

library typescript "@acme/pricing"
library python "acme-pricing"
library go "example.com/acme/pricing-go"

require tax path "../tax"
```

- `library typescript|python|go "<名前>"` は、その言語のレジストリでのパッケージの名前です。npm のパッケージの名前、Python のディストリビューションの名前、Go のモジュールのパスを書きます。Python では、ディストリビューション `acme-pricing` を `acme_pricing` として import します。名前を小文字にし、`-`・`_`・`.` の連なりを一つの `_` にした名前です。版 2 からは、Go の決まりに従って Go のモジュールのパスの最後に `/v2` が付き、ritsu はそれを import するコードもそのパスで書きます。
- `version` は、どの言語のライブラリの版にもなります。`--library` には `version` の行が要ります。Python の版（PEP 440）にできるプレリリースは、`alpha`、`beta`、`rc` に番号を付けたものだけです。`1.0.0-rc.1` は `1.0.0rc1` になります。`1.0.0-dev.1` なら、ritsu は止まります。
- `description` と `license` は、package.json と pyproject.toml に入ります。`source` は package.json の `repository` に入り、npm の provenance がこれを確かめます（[npm に公開する](#npm-に公開する)）。`source` は pyproject.toml の `[project.urls]` の `Repository` にも入ります。`source` はパッケージと一緒に公開されるので、ユーザー、パスワード、クエリを持つ URL は E302 になります。

## 生成する

<!-- run in pricing -->
```console
$ ritsu gen --library --out ../out/pricing --lang ja
生成しました: ../out/pricing/typescript/rules/order_total.ts
…
生成しました: ../out/pricing/python/pyproject.toml
…
生成しました: ../out/pricing/go/go.mod
```

`ritsu gen --library` は、`library` の行のある言語ごとに、`--out` の下の `typescript/`、`python/`、`go/` にライブラリを書きます。`--target typescript|python|go` を付ければ、その一つだけを書きます。ライブラリはモジュール全体（`exclude` の行が外すものを除く）なので、`--library` にはパスを渡せず、`--name` と `--module` も付けられません。ライブラリの名前は `library` の行から取ります。`--check` は、`--library` を付けないときと同じく、いまあるものとの違いを言います。

## 依存は依存のライブラリから読む

`shop` は `pricing` を require します。`shop` のコードは、`pricing` の規則を `pricing` のライブラリから、`pricing` の ritsu.mod が言語ごとに付けた名前で import します。

<!-- out/shop/typescript/glue/basket.ts -->
```ts
import * as rule_pricing_order_total from "@acme/pricing/rules/order_total";
```

<!-- out/shop/python/acme_shop/glue/basket.py -->
```python
from acme_pricing.rules import order_total as _rule_pricing_order_total
```

<!-- out/shop/go/glue/basket/basket.go -->
```go
pricingordertotal "example.com/acme/pricing-go/rules/ordertotal"
```

マニフェストに書くのは、ライブラリのコードが import するものだけです。`pricing` の注文の合計は `tax` の規則を当てますが、その規則はコードの中に組み込まれます。そのため `pricing` のコードは `tax` から何も import せず、`pricing` のマニフェストにも `tax` は載りません。

それでも、どの依存にも、その依存自身のライブラリか `--embed` が要ります。依存の ritsu.mod に、ある言語の `library` の行が無ければ、ritsu はその依存を挙げて止まります。

<!-- run in pricing, with no library lines in tax's ritsu.mod -->
```console
$ ritsu gen --library --out ../out/pricing --lang ja
エラー: 依存 `tax`（example.com/acme/tax）の ritsu.mod に、`library typescript` の行がありません。ライブラリの形のパッケージは、依存の部分を依存のライブラリから読みます。その ritsu.mod に `library typescript "<名前>"` を書くか、`--embed tax` を付けて、このパッケージの deps/ に入れてください
```

`--embed tax` を付けると、`pricing` のライブラリは `tax` のコードを `deps/tax/` に持ちます。`--library` を付けないときのパッケージと同じ持ち方です。このとき、`pricing` を通してだけ `tax` に届く `shop` は、`tax` を `pricing` のライブラリから読み、`shop` で `--embed tax` を付けると止まります。ライブラリを通してだけ届くモジュールは、そのライブラリが持つからです。

### 範囲

ライブラリは、import するライブラリを、モジュールの木が選んだ版からの範囲で求めます。ランタイムは、ritsu が試した版からの範囲で求めます。範囲は次のメジャー版の手前までで、版が `0.y.z` なら次のマイナー版の手前までです。

| 選んだ版 | npm | Python | Go |
|---|---|---|---|
| `1.1.0` | `^1.1.0` | `>=1.1.0,<2.0.0` | `v1.1.0` |
| `0.17.9` | `^0.17.9` | `>=0.17.9,<0.18.0` | `v0.17.9` |
| `0.0.3` | `>=0.0.3 <0.1.0` | `>=0.0.3,<0.1.0` | `v0.0.3` |

npm の `^0.0.3` は `0.0.3` しか受け取らないので、ritsu はこの範囲だけ上限まで書きます。Go には、ritsu は範囲ではなく版を書きます。Go は `require` を、そのモジュールが受け取るいちばん低い版として読み、各モジュールが求める版のうちいちばん高いものでビルドするからです。

`--library` を付けないパッケージには、ritsu は範囲ではなく、試した版そのものを書きます。アプリケーションは、試した版でそのまま動かすものだからです。ライブラリは、アプリケーションのいくつもの依存の一つです。二つのライブラリが同じランタイムを別々の版に固定すれば、Python は両方を入れられず、npm は二つのコピーを入れます。そこでライブラリは範囲を書き、範囲の中のどの版にするかは、アプリケーションのロックファイルが決めます。`shop` は `pricing` と Temporal の SDK を import します。

<!-- out/shop/typescript/package.json -->
```json
  "dependencies": {
    "@temporalio/activity": "^1.24.0",
    "@temporalio/client": "^1.24.0",
    "@temporalio/common": "^1.24.0",
    "@temporalio/worker": "^1.24.0",
    "@temporalio/workflow": "^1.24.0"
  },
  "peerDependencies": {
    "@acme/pricing": "^1.1.0"
  },
```

<!-- out/shop/python/pyproject.toml -->
```toml
dependencies = [
  "acme-pricing>=1.1.0,<2.0.0",
  "temporalio>=1.33.0,<2.0.0",
]
```

<!-- out/shop/go/go.mod -->
```text
require (
	example.com/acme/pricing-go v1.1.0
	go.temporal.io/api v1.63.5
	go.temporal.io/sdk v1.49.0
)
```

npm では、ritsu は、ライブラリが import するライブラリを `peerDependencies` に書きます。アプリケーションが、自分の分も、`pricing` を import するどのライブラリの分も、一つの `pricing` で済ませるためです。

## 単位の型はライブラリをまたいで一つ

`shop` のお金の額は、`pricing` の型です。`shop` の単位のモジュールは、型を `pricing` のライブラリから読み込み、同じ名前で公開します。

<!-- out/shop/typescript/units.ts -->
```ts
import type { GBPc } from "@acme/pricing/units";

export type { GBPc }; // money[GBPc]
```

<!-- out/shop/python/acme_shop/units.py -->
```python
from acme_pricing.units import GBPc as GBPc  # money[GBPc]
```

<!-- out/shop/go/units/units.go -->
```go
import pricinggounits "example.com/acme/pricing-go/units"

// GBPc is an amount of money[GBPc], the type of example.com/acme/pricing-go/units.
type GBPc = pricinggounits.GBPc
```

`shop` のコードが返す額は、変換せずに `pricing` の規則に渡せます。それでも、各言語の型の検査は、ほかの単位の額を通しません。依存の関係の無い二つのライブラリが同じ単位を持っていれば、両方を import するパッケージの中に、その単位の型が二つできます。そのため ritsu は止まります。片方のライブラリをもう片方に依存させるか、`--library` を付けずに生成してください。

## レジストリごとの中身

### npm

<!-- out/pricing/typescript/package.json -->
```json
{
  "name": "@acme/pricing",
  "version": "1.1.0",
  "description": "Shipping fees and order totals of the Acme shop",
  "license": "MIT OR Apache-2.0",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/acme/pricing.git"
  },
  "type": "commonjs",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "default": "./dist/index.js"
    },
    "./*": {
      "types": "./dist/*.d.ts",
      "default": "./dist/*.js"
    }
  },
  "files": [
    "dist"
  ],
  "scripts": {
    "prepare": "tsc -p tsconfig.json"
  },
  "devDependencies": {
    "typescript": "7.0.2"
  }
}
```

- 公開するのは、組んだあとのライブラリです。npm は、ディレクトリを入れるときと、パッケージにまとめる前や公開する前に、`prepare`（横の tsconfig.json での `tsc`）を走らせます。パッケージに入るのは `dist/`（JavaScript とその宣言）だけです。Node は node_modules の下の `.ts` の型を外さないので、TypeScript のままでは使えません。
- ライブラリは CommonJS で、ES モジュールからも import できます。生成したコードの相対の import には拡張子が無く、Node の ES モジュールでは拡張子が要るからです。
- `description` は、モジュールの `description` の行です。npm は、これをパッケージのページに出します。
- 最初のキー `"//"`（上の例では省きました）には、ritsu が生成したファイルであることを示す一文 `Code generated by ritsu <版>. DO NOT EDIT.` が入ります。JSON には注が書けないので、キーに書きます。npm はこのキーを読み飛ばします。`ritsu gen` は、ほかの生成したファイルでは頭の注にあるこの一文を、package.json ではこのキーから読みます。
- `optional-modules.d.ts` は、コードが頼まれたときだけ読み込む SDK を、型の無いモジュールとして宣言します（ワークフローのエージェントは、モデルの SDK を `await import(…)` で読み込みます）。そのため、ライブラリはそれらを入れずに組めます。使うアプリケーションが、頼むものだけを入れます。

### PyPI

<!-- out/pricing/python/pyproject.toml -->
```toml
[project]
name = "acme-pricing"
version = "1.1.0"
description = "Shipping fees and order totals of the Acme shop"
license = "MIT OR Apache-2.0"
requires-python = ">=3.10"
dependencies = [
]

[project.urls]
Repository = "https://github.com/acme/pricing"

[build-system]
requires = ["setuptools>=77"]
build-backend = "setuptools.build_meta"
```

- `requires-python` は、`--library` を付けないパッケージと同じです。setuptools は、77 から `license` を SPDX の式（PEP 639）として読みます。
- `Repository` は、モジュールの `source` の行です。ritsu は、モジュールに `source` の行があるときだけこれを書きます。そのリポジトリの GitHub Actions から信頼された発行者（[PyPI に公開する](#pypi-に公開する)）としてアップロードすると、PyPI はこの URL を確かめたものとして示します。

### Go

<!-- out/shop/go/go.mod -->
```text
module example.com/acme/shop-go

go 1.26.0
```

- `go` の行は、サポートが続いている Go のリリースのうち、いちばん古いものです（Go は新しい二つのメジャーリリースをサポートします）。ライブラリが require するランタイムのモジュールがそれより新しい版を求めれば、ritsu はその版を書きます。
- ritsu は go.sum を書きません。ライブラリを使うコードが、`go get` を走らせたときに、自分の go.sum にモジュールのハッシュを書きます。
- 生成したディレクトリは、それだけではビルドできません。go.mod は、ライブラリが import するモジュールを書きますが、そのモジュールがさらに import するものは書かないからです（Go は両方を求めます）。それでも、そこで `go mod tidy` を走らせないでください。`go mod tidy` は残りを go.mod に書き足します。すると `ritsu gen --check` は go.mod が古いと言い、次の `ritsu gen` は書き足したものを取り除きます。タグを付ける前にライブラリをビルドして確かめるときは、別のモジュールからそのライブラリを require して `replace` で生成したディレクトリを指し、そのモジュールで `go vet -mod=mod example.com/acme/pricing-go/...` を走らせてください。
- ライブラリ自身のリポジトリの CI では、`go build -mod=mod ./...`（または `go vet -mod=mod ./...`）で組んでください。Go は、組むのに要るものを CI の手元の go.mod と go.sum に書き足します。そのどちらもコミットしないでください。

## 公開する

ライブラリは、モジュールが require する順に公開してください。`tax`、`pricing`、`shop` の順です。ライブラリは依存を、木が選んだ版から求めるので、その版が先にレジストリになければなりません。npm は、ライブラリを組むときにもそれを入れます。

下のワークフローは、モジュールのリポジトリ（`source` の行のもの）で、モジュールの版のタグを付けたときに走ります。どちらも、ritsu を GitHub Action で入れ、その場でライブラリを生成します。ritsu のテストは、ライブラリを各レジストリの道具と同じやり方で組みますが、ワークフローそのものは走らせません。

### npm に公開する

GitHub Actions から OIDC のトークン（`id-token: write`）で公開すると、npm はパッケージに provenance を付けます。provenance は、パッケージを組んだリポジトリとワークフローを示す、署名つきの記録です。npm は、package.json の `repository` がそのリポジトリと一致すること（大文字と小文字の違いも認めません）を求めます。ritsu は、`repository` を `source` の行から、npm が公開するときに直したあとの形で書きます。GitHub、GitLab、Bitbucket、sourcehut のリポジトリと gist の URL は、npm の形になります。たとえば `https://github.com/acme/pricing` は `git+https://github.com/acme/pricing.git` になります。npm はこの形をそのまま公開し、package.json を直したという警告も出しません。

npmjs.com でパッケージに信頼された発行者（trusted publisher）を設定すれば（npm 11.5.1 以上、Node 22.14.0 以上）、ワークフローにトークンは要らず、npm が provenance を自分で付けます。

```yaml
name: Publish the npm package
on:
  push:
    tags: ["v*"]
jobs:
  publish:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      id-token: write
    steps:
      - uses: actions/checkout@v6
      - uses: i2y/ritsu@v0.37.0
      - uses: actions/setup-node@v6
        with:
          node-version: "24"
          registry-url: "https://registry.npmjs.org"
          package-manager-cache: false
      - run: ritsu mod fetch
      - run: ritsu gen --library --target typescript --out lib
      - run: npm install
        working-directory: lib/typescript
      - run: npm publish
        working-directory: lib/typescript
```

`ritsu mod fetch` は、ロックファイルが固定するモジュールのうち、まだ手元に無いものを取ってきます。`npm install` は、TypeScript と、パッケージが import するライブラリを入れ、`dist/` を組みます。`npm publish` は、もう一度組んでから公開します。信頼された発行者の代わりにトークンを使うなら、最後の手順は次のとおりです（パッケージを初めて公開するときは `--access public` が要ります）。

```yaml
      - run: npm publish --provenance --access public
        working-directory: lib/typescript
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

### PyPI に公開する

PyPI の信頼された発行者は、トークンなしで、ワークフローからリリースを受け取ります。PyPI で、プロジェクトに発行者を設定してください。設定に書くのは、リポジトリ、ワークフローのファイル名、環境の名前 `pypi` です。PyPI にまだ無いプロジェクトなら、保留中の発行者（pending publisher）が、最初のアップロードのときにプロジェクトを作ります。Python Packaging Authority のガイドのとおり、一つのジョブがディストリビューションを組み、OIDC のトークンを受け取る別のジョブだけが公開します。

```yaml
name: Publish the Python package
on:
  push:
    tags: ["v*"]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          persist-credentials: false
      - uses: i2y/ritsu@v0.37.0
      - uses: actions/setup-python@v6
        with:
          python-version: "3.x"
      - run: ritsu mod fetch
      - run: ritsu gen --library --target python --out lib
      - run: python3 -m pip install build --user
      - run: python3 -m build lib/python
      - uses: actions/upload-artifact@v5
        with:
          name: python-package-distributions
          path: lib/python/dist/
  publish:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: pypi
      url: https://pypi.org/p/acme-pricing
    permissions:
      id-token: write
    steps:
      - uses: actions/download-artifact@v6
        with:
          name: python-package-distributions
          path: dist/
      - uses: pypa/gh-action-pypi-publish@release/v1
```

このアクションは、ディストリビューションと一緒に、その attestation（どこで組んだかを示す署名つきの記録）も標準でアップロードします。

### Go で公開する

Go は、モジュールのパスにあるリポジトリから、版のタグでモジュールを取ります。アップロードするものはありません。そのリポジトリのクローンにライブラリを生成し、そこでタグを付けてください。下の `lib/go` は、`example.com/acme/pricing-go` のリポジトリのクローンです。

```console
$ ritsu gen --library --target go --out lib
$ cd lib/go
$ git add -A
$ git commit -m "pricing-go v1.1.0"
$ git tag v1.1.0
$ git push origin HEAD v1.1.0
$ GOPROXY=proxy.golang.org go list -m example.com/acme/pricing-go@v1.1.0
```

最後のコマンドは、Go の公開の手引きと同じく、Go のモジュールプロキシにその版を取りに行かせます。モジュールがリポジトリのサブディレクトリにあるときは、サブディレクトリの名前がモジュールのパスに入り、タグの頭にも付きます。リポジトリ `example.com/acme/pricing` のディレクトリ `go` にあるモジュール `example.com/acme/pricing/go` のタグは、`go/v1.1.0` です。一度 push したタグは動かさないでください。Go は、ある版を取るたびに最初に取ったものと照らし合わせ、違えばセキュリティのエラーにします。

## レジストリの道具が書くもの

ライブラリの横には、tsc が `dist/` を、setuptools が `build/` と `<名前>.egg-info/` を、Go が `go.sum` を書きます。`--library` を付けたときの `ritsu gen --check` と次の `ritsu gen` は、ライブラリのディレクトリの直下にあるこれらを、そのまま残します。tsc と setuptools は、ritsu が生成したことを言うファイルの最初の行もコピーしますが、それでも残します。

## ライブラリではできないこと

- **import するライブラリの規則に寄与すること**。ライブラリのコードは、生成したときにモジュールにあった寄与で組まれていて、使う側のコードからの寄与は受けられません。パッケージの寄与が、ライブラリの持つ規則に寄与していれば、ritsu は止まります。`--library` を付けずに生成して、寄与を合わせた規則をパッケージに持たせるか、寄与をライブラリの側のモジュールに移してください。
- **ライブラリをまたいで kaname の帳簿の型を渡すこと**。kaname が帳簿に渡す型（`glue/books/`）は、パッケージが自分で持ちます。同じ帳簿を持つライブラリは、その型を別に持ちます。Python と Go はそれらを別の型として扱い（TypeScript は型を形で比べるので、一つの型として扱います）、`KanameError` もライブラリごとに別のものになります。モジュールの間で帳簿の値を渡すなら、`--library` を付けずに、一つのパッケージとして生成してください。
- **依存の関係の無い二つのライブラリに、同じ単位を持たせること**。理由と代わりの手は、[単位の型はライブラリをまたいで一つ](#単位の型はライブラリをまたいで一つ)に書きました。
