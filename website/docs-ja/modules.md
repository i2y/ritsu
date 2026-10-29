# モジュール

モジュールは `ritsu.mod` を持つディレクトリです。ほかのプロジェクトが、横のディレクトリ、git のリポジトリ、OCI のレジストリのどれかから足して、そのファイルを `@<名前>/…` のパスで読みます。ritsu のリポジトリから配るモジュールは、[モジュールの一覧](modules/list/)に、版、ファイルごとのページ、版のあいだの差とともに載せています。

プロジェクトは、依存のファイルを、自分のファイルと一緒に、各言語の検査と言語をまたぐ検査にかけます。そのため、証明はモジュールの境目でも切れません。このページの例は、ritsu のテストの三つのモジュール [crates/ritsu/tests/projects/modules.ja](https://github.com/i2y/ritsu/tree/main/crates/ritsu/tests/projects/modules.ja) です。`tax`（ディレクトリは `税`）は付加価値税の税率を、`pricing`（`価格`）は送料と注文の合計を持ちます。注文の合計は `tax` の税率を準用します。`shop`（`店`）は注文のフローとかごを持ち、その値段を `pricing` で決めます。

## モジュールを書く

`ritsu mod init <名前>` は、走らせたディレクトリに `ritsu.mod` を書きます。そこから、そのディレクトリはモジュールになります。次は `pricing` の `ritsu.mod` です。

<!-- crates/ritsu/tests/projects/modules.ja/価格/ritsu.mod -->
```text
# Acme の店の値段。送料と、tax の税率を入れた注文の合計。
module example.com/acme/pricing
version 1.1.0
description "Acme の店の送料と注文の合計"
license "MIT OR Apache-2.0"

require tax path "../税"
```

`ritsu.mod` は一行に一つの文を書きます。`#` から行の終わりまでは注です。最初の文は `module` です。

| 行 | 意味 |
|---|---|
| `module <名前>` | モジュールの名前。レジストリに送り出すモジュールは、そのレジストリのリポジトリで名前を付けます（`example.com/acme/pricing`、`ghcr.io/i2y/ritsu-holidays`）。git で共有するモジュールの名前は、置いてある場所です（`example.com/acme/rates`。[下](#git-でモジュールを共有する)）。パスでだけ読むモジュールは、短い名前（`shop`）でもかまいません。 |
| `version <版>` | 版。SemVer の形で、頭に `v` を付けず、ビルドのメタデータも付けません（`1.4.0`、`2.0.0-rc.1`）。送り出すときのタグになります。git で共有するモジュールのタグは、頭に `v` を付けたものです（`v1.4.0`）。 |
| `ritsu >=<版>` | このモジュールを読める ritsu の下限。古い ritsu は、要る版を言って止まります。 |
| `description`、`license`、`source` | レジストリ、モジュールの一覧、ライブラリが見せるもの。`license` は SPDX の式です。`source` はモジュールのリポジトリの https の URL で、公開するものなので、ユーザー、パスワード、クエリは書けません（E302）。 |
| `require <名前> path "<ディレクトリ>"` | ディレクトリにある依存。パスはモジュールのルートから書きます。 |
| `require <名前> oci "<リポジトリ>" <版>` | レジストリにある依存。その版か、それより新しい版を使います。 |
| `require <名前> git "<URL>" <版> [dir "<ディレクトリ>"]` | git のリポジトリのタグにある依存。その版か、それより新しい版を使います。`dir` は、リポジトリの中のモジュールのディレクトリです（[git でモジュールを共有する](#git-でモジュールを共有する)）。 |
| `exclude "<パス>"` | `ritsu mod push` が送らないファイルかディレクトリ。`ritsu mod init` は、`ritsu gen` の出力先の `exclude "generated"` を書きます。 |
| `signature "<名前>" …` | その名前のモジュールに求める署名（[署名](#署名)）。 |
| `library typescript\|python\|go "<名前>"` | その言語でのモジュールのライブラリの名前（[ライブラリ](#ライブラリ)）。 |

依存の名前は `require` の次の語で、英小文字、数字、`_` で書き、英字で始めます。モジュールのファイルは、依存のファイルを、`@` とこの名前に、依存のルートからのパスを続けて指します。`pricing` の `注文の合計.rule` は、`tax` の税率を準用します。

```rule
rule 注文の合計(order_total) v1
…
apply 付加価値税(vat) = "@tax/規則/付加価値税率.rule" meaning:35d5c7d29edf24c8
```

`@` のパスは、ファイルがどのディレクトリにあっても、モジュールのルートから読みます。どの言語も、パスを読むところでは `@` のパスを読みます。規則の `apply` と `import`、ワークフローの `use rule`、カレンダーの `use calendar`、ゲートの `use`、kaname の `use`、yuen のリンク、sakai のコンテキストです。モジュールの中でコマンドラインに渡したパスも同じに読みます。`ritsu rulec doc @tax/規則/付加価値税率.rule` は依存の規則のページを書き、`ritsu mod where` はそのファイルがディスクのどこにあるかを言います。

- モジュールのルートは、`ritsu.mod` のあるディレクトリです。ritsu は、`--root` が無ければ、渡したファイルの上で `.git` か `ritsu.mod` を持つ一番近いディレクトリをルートにします。
- パスはモジュールの中に留まります。モジュールのルートの外へ出るパスと、下のモジュールの中へ `@` を通らずに入るパスは、そのパスを読む言語のエラーです。ルートに、名前が `@` で始まるファイルかディレクトリがあるのもエラーです（E307）。
- 下のディレクトリで自分の `ritsu.mod` を持つものは、別のモジュールです。モジュールを歩くときは、その中に入りません。上から走らせた `ritsu check` は、それだけを検査する方法を言います。
- `ritsu.mod` の無いプロジェクトでは、何も変わりません。`@` で始まるパスも、ほかのパスと同じです。

## モジュールを足す

`ritsu mod add <名前> <取り先>` で依存を足します。ディレクトリなら、走らせた場所からの `./…` か `../…` で、レジストリなら `<リポジトリ>@<版>` か `<リポジトリ>@latest` で、git のリポジトリなら `<URL>@<版>` か `<URL>@latest` で書きます（`@latest` か、版を書かないときは、プレリリースでない一番新しい版です）。最後の `require` の行のあとに一行書き、モジュールに lock が要れば `ritsu.lock` も書きます。

```console
$ ritsu mod add pricing ../価格
$ ritsu mod add holidays ghcr.io/i2y/ritsu-holidays@1.0.0
$ ritsu mod add rates https://example.com/acme/rates@1.2.0
$ ritsu mod get holidays@latest
$ ritsu mod remove holidays
```

名前は、依存を足す側が決めます。プロジェクトのファイルが `@` のあとに書く名前です。このサイトの[モジュールの一覧](modules/list/)の `ritsu mod add` の行は、祝日のモジュールをリポジトリの名前から `ritsu_holidays` と呼びますが、このページは、モジュールの README と同じく `holidays` と呼びます。

`ritsu mod get <名前>@<版>` はレジストリか git のリポジトリから取る依存の版を変え、`ritsu mod remove <名前>` は依存の行を消します。三つのコマンドは、`ritsu.mod` と `ritsu.lock` をそろって書くか、どちらも書かないかのどちらかで、`ritsu.mod` のほかの行と注はそのまま残します。

`ritsu.lock` は ritsu が書くファイルです。プロジェクトが読むモジュールに、依存の依存まで、一行ずつを持ちます。レジストリのモジュールの行は、選んだ版と二つのダイジェストを持ちます。レジストリと cosign が使う OCI のマニフェストのダイジェストと、モジュールのファイルのダイジェストです。git のモジュールの行は、同じものに加えて、タグが指していたコミットとリポジトリの URL を持ちます。版は最小の版の選び方（Minimal Version Selection）で選びます。モジュールごとに、`require` の行が求める版のうち一番高いものを選び、一つのモジュールには一つの版だけを使います。そのため、lock は、レジストリに問い合わせずに `ritsu.mod` だけから決まり、同じファイルを持つ二つの機械は、どのモジュールでも同じファイルを読みます。レジストリからも git のリポジトリからも取る依存の無いモジュールには、lock は要らず、作りません。

- どのコマンドも lock を `ritsu.mod` と突き合わせ、合わなければ止まります（E304）。`ritsu mod lock` が lock を書き直し、`ritsu mod lock --check` は書き直しになるときに exit 1 で終わります。CI の一段に使えます。
- レジストリと git のリポジトリから取る依存のファイルは、読むたびに lock のダイジェストと突き合わせます（E305）。
- 作業ツリーの `require … path` は、木のどこにある同じモジュールの `require … oci` と `require … git` よりも優先します。依存を直しながら、使う側のモジュールで確かめるときは、この行を書きます。

`ritsu mod list` は、プロジェクトが読むモジュールを並べます。

```console
$ cd crates/ritsu/tests/projects/modules.ja/店
$ ritsu mod list --lang ja
モジュール example.com/acme/shop 0.1.0
@pricing       example.com/acme/pricing 1.1.0  パス ../価格
@pricing/@tax  example.com/acme/tax 1.0.0      パス ../税
ritsu mod list: 依存 2 個（パス 2 個、レジストリ 0 個）。lock は要りません
```

モジュールの中の `ritsu check` は、プロジェクトが読むモジュールを全部読み、そのファイルを、言語をまたぐ検査も含めて、自分のファイルと同じに検査します。

```console
$ cd crates/ritsu/tests/projects/modules.ja/店
$ ritsu check . --lang ja
ok 会計/かご.kn: 関数 2 個、total
注文/注文を受ける.flow: 検査を通りました
依存: モジュール 2 個、ファイル 3 個（rulec 3）。どれも検査を通りました
ritsu check: ファイル 2 個（kaname 1、dandori 1）。どれも検査を通りました。言語の境目: 確かめた 0 か所、決められない 0 か所
```

- 依存のファイルのエラーは、検査を落とします。依存のファイルの警告は依存の行に数え、一つずつは `--deps-warnings` で出します。
- 足した依存のファイルを、モジュールのどのファイルも指さなければ W301 です。依存の依存（`@pricing/@tax/…`）を指すファイルは W303 です。そのモジュールを自分の `ritsu.mod` で `require` してください。
- 依存の `.geas` の主張は走らせません。主張は依存の作者のプログラムについてのもので、そのプログラムは手元の機械にありません。
- 依存の規則への `contribution` は、プロジェクトの中でその規則に足されます。`ritsu check` は、モジュールの境目を越えるものを一つずつ注で言います。

## 読むモジュールを取り出す

通信するのは `ritsu mod` だけです。`ritsu check`、`gen`、`run`、各言語のコマンドは、レジストリと git のリポジトリから取る依存を、`ritsu mod fetch` が置いたところから読みます。そこに無ければ、`ritsu mod fetch` を走らせるよう言って止まります。

```console
$ ritsu mod fetch
$ ritsu mod vendor
```

- `ritsu mod fetch` は、lock のモジュールのうち手元に無いものを取り出し、置き場に置きます。置き場は実行をまたいで使うディレクトリで、`RITSU_MODULES` が無ければ、macOS では `~/Library/Caches/ritsu/modules`、ほかでは `$XDG_CACHE_HOME/ritsu/modules`（無ければ `~/.cache/ritsu/modules`）です。ダイジェストと、`signature` の行が求める署名を確かめ、全部のモジュールが通ったときだけ書きます。`--rev <git の版>` はその版の lock が固定するものを、`<名前>@<版>` はその版を取り出します。
- `ritsu mod vendor` は、そのコピーをモジュールの中（`.ritsu/modules/`）に置きます。レジストリやリポジトリに届かないビルドや、依存をリポジトリに入れておくときに使います。`ritsu mod vendor --check` は、コピーが lock に合うかを確かめます。どのコマンドも、置き場より先にこのコピーを読みます。
- `ritsu mod clean` は置き場の中身を消します。`--keep <lock>…` を付ければ、その lock が固定していないものだけを消します。消したものは、ダイジェストでいつでも取り直せます。
- `ritsu mod where @<名前>/<パス>` は、依存のファイルがディスクのどこにあるかを言います。エディタで開くときに使います。

レジストリには curl で HTTPS で問い合わせます（平文の HTTP は、この機械の上のレジストリだけです）。最初の問い合わせは資格情報なしで送ります。レジストリが資格情報を求めれば、ritsu は Docker の設定ファイル（`$DOCKER_CONFIG/config.json`、無ければ `~/.docker/config.json`）を、`docker login` と資格情報のヘルパーが書いた形で読み、そこにあるヘルパーを走らせます。資格情報とトークンは、どの出力、ログ、ファイルにも出しません。

npm のパッケージは、置き場と同梱のコピーを、ネイティブの ritsu と同じに読みます。レジストリに問い合わせるコマンドと、git か cosign を走らせるコマンド（`add`、`get`、`fetch`、`versions`、`push`、`report`、`site` と、取り出しが要るときの `lock`、`remove`、`diff`）は、ネイティブの ritsu を使うよう言って止まります。API の `dirs` で開くディレクトリを絞るときは、置き場を `dirs` に足すか、`ritsu mod vendor` で同梱のコピーをモジュールに置きます。

## git でモジュールを共有する

自分たちのリポジトリのあいだで共有するモジュールには、レジストリが要りません。作者がコミットにタグ `v<版>` を打って push し、使う側はリポジトリの URL で依存に足します。リポジトリの中のディレクトリにあるモジュールは `--dir` で指し、タグは `<ディレクトリ>/v<版>` です。

```console
$ ritsu mod add rates https://example.com/acme/rates@1.2.0
$ ritsu mod add tax git@example.com:acme/mono.git@1.0.0 --dir modules/tax
$ ritsu mod versions https://example.com/acme/rates
```

```text
require rates git "https://example.com/acme/rates" 1.2.0
require tax git "git@example.com:acme/mono.git" 1.0.0 dir "modules/tax"
```

- URL は `https://…`、`ssh://…`、scp の形の `<ユーザー>@<ホスト>:<パス>` のどれかです。`git://`、`file://`、ほかの機械への平文の `http://`、`<transport>::…`、パスワードかクエリを含む URL は E302 です。
- モジュールの名前は、置いてある場所です。ホストと、`.git` を除いたリポジトリのパスと、`dir` をつなぎ、スキーム、ユーザー、ポートは入れません（`example.com/acme/rates`、`example.com/acme/mono/modules/tax`）。タグの `ritsu.mod` には、この名前の `module` と、タグの版を書きます。
- 名前は小文字なので、URL も小文字で書いてください。多くのホストは、リポジトリのパスの大文字と小文字を区別しません。区別するホストで、パスに大文字を含むリポジトリは、依存に足せません。
- `v` の無いタグ、`+` を含むタグ、SemVer の形でないタグは、版になりません。プレリリースは版になりますが、`@latest` では選びません。
- lock は、タグが指していたコミットを、URL と一緒に固定します。`ritsu mod fetch` は、そのコミットを名前で取り、チェックアウトはしません。コミットと木を取り、モジュールのファイルをまとめて取って、git が持つバイトのまま読みます。filter も改行の変換もフックも効きません。モジュールの中にシンボリックリンク、サブモジュール、Git LFS のポインタがあれば止まります。
- タグが動いたときは、レジストリのタグと同じに扱います。`ritsu mod fetch` は lock のコミットを取り続け、`ritsu mod get` はタグがいま指すコミットを取り、`ritsu mod report` は動いたことを先頭に出します。リポジトリが lock のコミットをもう渡さなければ、`ritsu mod fetch` はタグが動いたと言って止まります。
- ritsu は、2.29 以上の git（`ritsu mod fetch --rev` と `ritsu mod report` では 2.30 以上）を、手元の設定のまま走らせます。ssh の鍵、資格情報のヘルパー、`url.<base>.insteadOf` は、`git fetch` と同じく効きます。git に端末でパスワードを聞かせないので、資格情報が無ければすぐに止まります。

git で共有するモジュールには、確かめる署名がありません。lock のコミットとダイジェストで固定し、そのモジュールに当たる `signature` の行は E309 です。git から取るモジュールを依存に持つモジュールは、レジストリに送り出せません。git から取るモジュールの名前は置いてある場所なので、一つのモジュールを配る方法はどちらか一つです。git で共有していたモジュールをレジストリに移すときは、`module` の行をレジストリのリポジトリに変えて、新しい版を送り出します。使う側は `require … git` の行を `require … oci` に書き換えて、`ritsu mod lock` を走らせます。ファイルは使う側が付けた名前でモジュールを指すので、変わりません。

GitHub Actions では、`actions/checkout` が持つトークンは、チェックアウトしたリポジトリのためのもので、ほかのリポジトリは読めません。ritsu は git を自分の一時のリポジトリで走らせるので、このトークンを使いません。ほかの非公開のリポジトリのモジュールを依存に持つジョブでは、そのリポジトリを読めるトークンをシークレット（ここでは `MODULES_TOKEN`）に置き、git の大域の設定で git に渡します。トークンを環境変数から読む資格情報のヘルパーにすれば、トークンはファイルに残りません。ユーザーの名前は、ホストがトークンと一緒に受け付けるものにします。

```sh
git config --global credential.https://example.com.username ci
git config --global credential.https://example.com.helper '!f() { test "$1" = get && echo "password=$MODULES_TOKEN"; }; f'
```

これを ritsu より前のステップで走らせ、`ritsu mod fetch` を走らせるステップの `env` に、シークレットを `MODULES_TOKEN` として渡してください。

## レジストリに送り出す

`ritsu mod push` は、モジュールの名前が指すリポジトリに、版をタグにして送り出します。

```console
$ cd modules/holidays
$ ritsu mod push --dry-run --lang ja
ghcr.io/i2y/ritsu-holidays 1.0.0 の成果物を作りました（`--dry-run` なので、送っていません）
…
```

1. 名前がレジストリの形で、`version` と `ritsu >=` の行があり、`require … path` も `require … git` も無く、lock が合っているモジュールだけを受け付けます。
2. モジュールのファイルを集めます。`exclude` の行が挙げるもの、名前が `.` で始まるもの、下のモジュールは入れません。鍵を持つファイルがあれば受け付けません（W901。ファイルと行を言い、鍵そのものは出しません）。
3. 集めたファイルのコピーを `ritsu check` にかけ、エラーがあれば受け付けません。モジュール自身の `.geas` の主張は、主張が走らせるプログラムのある作業ツリーでの `ritsu check` に任せます。
4. OCI の成果物を作ります。同じファイルからは、いつも同じバイトと同じダイジェストができるので、だれでもソースからダイジェストを作って比べられます。`--dry-run` はここで止まり、ダイジェストを出します。
5. タグに同じファイルがあれば、送り出し済みです。違うファイルがあれば、送り出しを受け付けません。版は動かさないので、版を上げてください。
6. レジストリに無いものを送り、参照（`<名前>@sha256:…`）と、署名する `cosign sign` のコマンドを出します。

先に `docker login` でログインしておきます。ritsu も cosign も、Docker が持つ資格情報を読みます。このリポジトリのモジュールを公開するワークフロー [.github/workflows/modules.yml](https://github.com/i2y/ritsu/blob/main/.github/workflows/modules.yml) は、モジュールを検査し、ghcr.io に送り出し、鍵を持たずに cosign で署名し、使う側のモジュールと同じ形で署名を確かめます。

`ritsu mod site <一覧のファイル> --out <ディレクトリ>` は、このサイトの[モジュールの一覧](modules/list/)のような、モジュールを並べるサイトを書きます。モジュールごと、版ごとのページ、ファイルごとのページ、一つ前の版からの差、`ritsu mod add` の行、版をブラウザで試すページで開くボタンを作ります。一覧のファイルに書く行は、`title`、`description`、`playground`、`module`（レジストリのモジュール。全部の版を取り出して確かめます）、`git`（git で共有するモジュール。`git "<URL>"` か `git "<URL>" dir "<ディレクトリ>"` と書き、全部のタグを取り出して確かめます）、`signature`（`ritsu.mod` と同じ）、`local`（まだ送り出していない作業ツリー）です。確かめを通らない版があれば exit 1、一覧のファイルが読めないか、レジストリかリポジトリに届かなければ exit 2 で終わります。

## 署名

ritsu は署名しません。`ritsu mod push` が出したダイジェストに、cosign で署名します。GitHub Actions で鍵を持たずに署名するなら、権限 `id-token: write` を付けて、次を走らせます。

```console
$ cosign sign --yes ghcr.io/i2y/ritsu-holidays@sha256:…
```

鍵で署名するなら `cosign sign --key cosign.key <名前>@sha256:…` です。依存を足す側のモジュールは、どの署名を受け付けるかを、`ritsu.mod` の `signature` の行に書きます。このリポジトリのモジュールなら、次の行です。

```text
signature "ghcr.io/i2y/ritsu-holidays" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

| `signature "<名前>" …` | cosign に渡すもの |
|---|---|
| `key "keys/acme.pub"` | `--key`。鍵のファイルはモジュールのルートから読みます |
| `key "keys/acme.pub" tlog off` | 同じ。透明性ログに載っていない署名も受け付けます |
| `by "<署名者>" issuer "<発行者>"` | `--certificate-identity` と `--certificate-oidc-issuer` |
| `by matching "<正規表現>" issuer "<発行者>"` | `--certificate-identity-regexp` と `--certificate-oidc-issuer` |

- `signature` の次の文字列は、モジュールの名前か、`/` で終わる名前の頭の部分（`"example.com/acme/"`）です。
- モジュールに効くのは、手元のモジュールの行と、そのモジュールを直に `require` しているモジュールの行です。効く行は全部通らなければなりません。
- `ritsu mod fetch` は、モジュールを置き場に置く前に PATH の cosign を走らせ、確かめたことを置き場に記録します。`ritsu check` は cosign を走らせません。効く行が記録に無ければ、`ritsu mod fetch` を走らせるよう言って止まります。
- ritsu が署名を確かめるのは、レジストリから取るモジュールだけです。git から取るモジュールに当たる行は E309 です。行の頭の部分を狭めるか、そのモジュールをレジストリから取ってください。

## 版を上げると何が変わるか

`ritsu mod diff <旧> [<新>]` は、モジュールの二つの版を比べます。それぞれの側は、`<モジュールの名前>@<版>`（木が git から取るモジュールなら、その git のリポジトリから取ります）、ディレクトリ、`.`（作業ツリー。`<新>` を書かないときはこれ）のどれかで、どちらかの側が読むモジュールが手元に無ければ取り出します。足したファイル、消したファイル、変わったファイルと、`require` の変わりを並べます。両方の版にある規則、日付のファイル、カレンダー、ゲートのそれぞれについて、答えが変わる入力を、試した点だけでなく入力の全体で求めて出し、ほかの入力は同じ答えになると言います。変わったファイルごとに、使う側のコードに見せる形の差を、使う側を壊しうるものから並べます。次の例では、`pricing` の 1.1.0 が、2,500g までの荷物の送料を下げています。

```console
$ cd crates/ritsu/tests/projects/modules.ja/価格
$ ritsu mod diff ../版/価格-1.0.0 . --lang ja
ritsu mod diff：example.com/acme/pricing 1.0.0 → 1.1.0
  古い版：../版/価格-1.0.0
  新しい版：.
…
振る舞い：
  規則/注文の合計.rule (rulec)：同じ答えです
  規則/送料.rule (rulec)：答えが変わります
…
      地域 = 国内  かつ  重さ >=2001g <=2500g
        料金: 695 → 395
        当たる行: 表 重さの段 行2 → 表 重さの段 行1
        例: 地域=国内, 重さ=2001
…
インターフェース：
  規則/送料.rule (rulec、api)：差が 11 件、うち使う側を壊しうるもの 1 件
    変わった（壊しうる）：/connect/enums/地域/values/その他の地域/number：2 → 3
…
```

壊しうる差は候補です。使う側を壊すかどうかは、使う側の検査が決めます。`rulec diff`、`koyomi diff`、`sekisho diff` も、版をこれまでと同じ書き方で指せるうえに、二つの書き方を受け付けます。`@<名前>/<パス>` は lock が固定する版の依存のファイルで、`@<名前>@<版>/<パス>` はモジュールのその版のファイルです。

`ritsu mod report --base <git の版>` は、依存を上げる PR が知りたいことに答えます。手元のファイルは head のままにして、base の依存で一度、head の依存で一度読み、二つの lock が固定するモジュールを取り出します。head で増えた診断と消えた診断を挙げ、変わったモジュールごとに、その差と、手元のファイルの答えのうち変わるものを言います。タグが動いたモジュール（同じ版なのにファイルが違うもの）は、いちばん上に出します。markdown は PR のコメントにする形です。

```yaml
# .github/workflows/ritsu-mod-report.yml
name: ritsu mod report
on:
  pull_request:
    paths: [ritsu.mod, ritsu.lock]
permissions:
  contents: read
  pull-requests: write
jobs:
  report:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - uses: i2y/ritsu@v0.37.0
      - name: Report what raising the dependencies changes
        env:
          GH_TOKEN: ${{ github.token }}
          PR: ${{ github.event.pull_request.number }}
        run: |
          code=0
          ritsu mod report --base "origin/$GITHUB_BASE_REF" --format markdown > report.md || code=$?
          ritsu mod report --base "origin/$GITHUB_BASE_REF" --format json > report.json || true
          if [ -s report.md ]; then gh pr comment "$PR" --body-file report.md; fi
          exit "$code"
      - uses: actions/upload-artifact@v7
        if: always()
        with:
          name: ritsu-mod-report
          path: report.json
```

- `fetch-depth: 0` は、`origin/<base>` を読めるように履歴を取ってくるためです。
- `ritsu mod report` は、head の検査にエラーがあれば exit 1、変わったモジュールを比べられなければ exit 2 で終わります。答えが変わったときにも落とすなら、`--fail-on-change` を付けます。
- モジュールがリポジトリのルートに無いときは、そのディレクトリを `paths` と `--root` に書きます。
- fork からの PR では、書き込めないトークンになるので、コメントは付きません。
- 報告を日本語で書くなら、`--lang ja` を付けます。

## ライブラリ

モジュールは、`ritsu gen --library` で npm、PyPI、Go 向けに公開できます（[ライブラリとして公開する](libraries.md)）。
