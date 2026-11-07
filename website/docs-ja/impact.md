# 変更の行き先

`ritsu impact --base <版>` は、プロジェクトの二つの版を比べて、答えが動いたかと変更が誰に関わるかを一つの報告にまとめます。`--route <file.rule>` を付けると、自分で書いた規則が変更の行き先を決めます。そのままマージする、持ち主が読む、直すまで止める、といった行き先です。変更を誰が書いたかは区別しません。

```
ritsu impact --base <版> [--head <版>] [--route <file.rule>]
             [--format text|markdown|json] [--budget <n>] [--root <dir>] [--lang ja|en]
```

head は `--head` で版を指さなければ作業ツリーです。根は `ritsu check` と同じ決め方で、`ritsu.mod` のないプロジェクトでも動きます。`ritsu impact` は主張を走らせず、何も書きません。片方の版はディスクにないことがありますし、報告のためにサービスを起こしたり記録を書き換えたりするものではないからです。

## 何を言うか

報告の中身は、どれも各言語がもともと答えるものです。`ritsu impact` はそれを集めるだけで、ファイルの意味について自分では何も言いません。

- **ファイル**：変わったファイルごとに、足した・消した・名前を変えた・変えたの別と、言語のファイル（仕様）、生成物（先頭の行に生成器が書いたとあるもの）、手書きのコード（拡張子で見る）、文書（`.md`、`.markdown`、`.rst`、`.adoc`。隠しディレクトリの中は除き、大文字の名前は README、CHANGELOG、CHANGES、CONTRIBUTING だけ）、その他の別を言います。手書きのコードは変わった行を数えます。
- **検査**：両方の版の `ritsu check`（`.geas` は読むだけで主張は走らせません）と、head にだけあるエラーと警告。行だけ動いた診断は新しいと数えません。
- **動いたもの**：二つの版について `ritsu mod diff` が言うこと。規則で答えが変わる入力のすべて、日付のファイルで変わる日付のすべて、ゲートで決定が変わる組み合わせのすべてです。読んでいるファイルの変更も入ります。読んでいる暦が変わればゲートも動き、日付の条件を持つゲートでも、関係のないファイルだけの変更では何も出ません。
- **持ち主**：変更についての `yuen affected`。届いた要件、その持ち主、どの要件にも届かない変更です。`.req` がないプロジェクトではその旨を言います。
- **主張**：変更についての `geas affected`。触った主張と、どの主張も走らない変更行です。コードに合う記録（`.geas/<名前>.map.jsonl`）がなければ「分からない」と言い、変更したコードの行の全部を、どの主張も走らない行に数えます。
- **生成物**：`ritsu.mod` に `target` の行があり head が作業ツリーのときの `ritsu gen --check`。それ以外は確かめていないと言います。
- **比べられないもの**：`.flow`、`.kn`、`.ctx`、`.geas`、`.proto` は振る舞いの差を出せないので、数えるだけで中身には踏み込みません。ほかに比べられなかったものや決められなかったものは、理由を添えて並べます。

報告の最初の一文が結論です。[`crates/ritsu/examples/impact.ja`](https://github.com/i2y/ritsu/tree/main/crates/ritsu/examples/impact.ja) の例で、`refund_band.ja.rule` の区分を 20 点から 30 点に動かし（`yuen review` で確認を記録して）走らせると、こう出ます。

```
ritsu impact：base → 作業ツリー

答えが動きました（rule_changes 1、allow_gained 1、policies_moved 2）。行き先は 経路 = 持ち主が読む、人が読む = true。持ち主は 経理。

変わったファイル：6 個（仕様 2、生成物 0、手書きのコード 0、文書 0、その他 4）。手書きのコードの変わった行 0
  変えた：refund_band.ja.rule (仕様, rulec)
  変えた：refunds.ja.req (仕様, yuen)
```

その下に、規則の変わった入力の範囲と、規則を読むゲートの組み合わせが続きます。

```
      点数 >=21 <=30
        区分: 多数 → 少数
        当たる行: 表 区分を決める 行2 → 表 区分を決める 行1
        例: 点数=21
```

```
      許可が増えた（拒否 → 許可）：1 通り
        効いたポリシー：なし → 係は期間内に少数を返金できる
        - 係を持つ職員、注文、大きさ：少数、期間内：はい（古い版では大きさ：多数）
```

## 行き先を決める規則

規則の入力の名前（または別名）が下の事実の名前で、ほかの規則と同じ検査を通ります。出力は全部出ます。例の規則はこうです。

```rule
define 動いた数 : number = 規則の変化 + 日付の変化 + 増えた許可 + 減った許可 + 動いたポリシー + 帳簿の変化 + インターフェースの変化 + 壊すもの + 動いた要件
define 読まれない数 : number = 決められない + 確かめていない生成物 + 中身を比べない + 主張が走らない行 + その他のファイル + 新しい警告

table 経路を決める
policy unique
| 生成物が古い | 新しいエラー | 動いた数 | 読まれない数 | -> 経路 : 行き先 |
| true         | -            | -        | -            | 止める           |
| false        | >=1          | -        | -            | 止める           |
| false        | 0            | >=1      | -            | 持ち主が読む     |
| false        | 0            | 0        | >=1          | 持ち主が読む     |
| false        | 0            | 0        | 0            | マージ           |

table 誰が読むか
policy unique
| 動いた数 | 経路                 | -> 人が読む : bool |
| >=1      | 持ち主が読む, 止める | true               |
| 0        | 持ち主が読む, 止める | true               |
| 0        | マージ               | false              |
```

二つ目の表が約束を持っています。「答えが動いた変更がマージへ行く」行がないので、一つ目の表がそこへ送ると `ritsu check` が E101 で止まります。検査はすべての入力を見るので、例に挙げた変更だけでなく、どの変更についてもこれが成り立ちます。

この規則がマージへ通すのは、報告が請け合えるものだけです。文書、答えが同じと示せた仕様、どの要件にも届かず足した行の全部を主張が走らせるコードです。報告が何も知らないものは人に回します。仕様が読む契約、ビルドのファイル、ワークフロー（`other_files`）、何も確かめていない生成物（`generated_unchecked`）です。要件がよりどころにする主張が走らせるコードも人に回します（`requirements_moved`）。主張が行を走らせても、その行が間違ったときに主張が気づくとは限らないからです。

事実を受け取れない規則は、比べる前に exit 2 で止めます。事実の名前でない入力（使える名前を全部並べます）や、事実と型が違う入力です。入力の範囲を外れる値は、名前と値と範囲を言うだけで丸めません。報告は最後まで出て経路は空、exit は 2 です。

## 事実

数は 0 以上の `number`、それ以外は `bool` で受けます。

| 事実 | 中身 |
|---|---|
| `spec_files`、`generated_files`、`code_files`、`doc_files`、`other_files` | 種類ごとの変わったファイルの数。名前を変えたものは一つ |
| `code_lines` | 手書きのコードの変わった行。空行は数えず、一行の編集は 1。差分に行がないファイル（名前を変えただけなど）も 1 |
| `new_errors`、`new_warnings` | head にだけある診断 |
| `rule_changes` | 規則の答えが変わる入力の範囲の数と、足した・消した規則の数 |
| `date_changes` | 変わる日付や暦の日の数と、足した・消した日付のファイルの数 |
| `allow_gained`、`allow_lost` | ゲートが新しく許可する、拒否するようになった組み合わせの数 |
| `policies_moved` | 答えは同じで効くポリシーが変わった組み合わせの数、片方の版にしかないものの有無、足した・消したゲートの数 |
| `book_changes` | `.book` が出すものの変更の数と、足した・消した帳簿の数 |
| `interface_changes`、`interface_breaks` | 使う側のコードから見える変更の数と、そのうち壊す可能性のあるものの数 |
| `undecided` | 比べられなかった、決められなかったものの数 |
| `opaque_specs` | 変わった `.flow`、`.kn`、`.ctx`、`.geas`、`.proto` の数。中身は比べません |
| `requirements_moved`、`requirements_unknown` | 動いた要件の数と、どの要件にも届かない仕様とコードの数 |
| `claims_touched`、`unclaimed_lines` | 触った主張の数と、主張が走ると分からない手書きのコードの変更行の数。走らない行、記録に文として載っていない足した行（表、定数など。一行だけのコメントは数えない）、差分に行がないファイル |
| `claims_known` | どの `.geas` にも変更に合う記録があるか |
| `generated_stale`、`generated_checked` | 古い生成物があるか、そもそも確かめたか |
| `generated_unchecked` | 変わった生成物のうち `ritsu gen --check` が確かめなかったものの数。生成物だと言っているのは先頭の行だけです |

## exit と JSON

- head にだけあるエラーがあれば 1。
- そうでなく、比べられないものや決められないものがあるか、規則が事実を受け取れなければ 2。引数の誤りと、検査を通らない規則も 2。
- ほかは 0。規則が決めた行き先で exit は変わりません。

引数の誤りと読めない規則のとき以外、JSON は最後まで出ます。

```
{ritsu, base, head, files, check, behavior, interface, requirements, claims, generated, undecided,
 facts: {<名前>: <値>}, route: {file, outputs} | null, markdown}
```

`route.outputs` は各出力を公開の名前で持ち、列挙の値も公開の名前です。規則の名前が日本語でも、パイプラインには同じ語が渡ります。`markdown` はコメント用の報告で、6 万字までに収め、`@` はすべてコードの中に入れています。

## プルリクエストで使う

一度走らせれば、コメントと行き先の両方が手に入ります。`jq` で読みます。

```yaml
# .github/workflows/ritsu-impact.yml
name: ritsu impact
on:
  pull_request:
permissions:
  contents: read
  pull-requests: write
jobs:
  impact:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - uses: i2y/ritsu@v0.44.0
      - name: Say what the change moved and where it goes
        env:
          GH_TOKEN: ${{ github.token }}
          PR: ${{ github.event.pull_request.number }}
        run: |
          base="origin/$GITHUB_BASE_REF"
          git show "$base:route.rule" > "$RUNNER_TEMP/route.rule"
          code=0
          ritsu impact --base "$base" --route "$RUNNER_TEMP/route.rule" --format json > impact.json || code=$?
          jq -r .markdown impact.json > impact.md
          gh pr comment "$PR" --body-file impact.md
          route=$(jq -r '.route.outputs.route // empty' impact.json)
          if [ -z "$route" ]; then exit "$code"; fi
          if [ "$route" = owner_reads ]; then gh pr edit "$PR" --add-label "owner reads"; fi
          if [ "$route" = stop ]; then exit 1; fi
```

- 規則は base から読むので、プルリクエストが自分の行き先を書き換えることはできません。`route.rule` の変更も、プロジェクトの中にあれば `rule_changes` に出ます。ワークフローを足す前に、規則を先にマージしてください。
- `fetch-depth: 0` は履歴を取るためのもので、`origin/<base>` を読めるようにします。
- `ritsu.mod` があるプロジェクトでは、`ritsu mod report` と同じく、両方のロックが固定したモジュールを取得します。一つのリポジトリに複数のモジュールがあるときは、それぞれのディレクトリを `--root` に渡してください。根より下にある別のモジュールの変更は、比べていないものとして並びます。
- ラベルはリポジトリに作っておきます。フォークからのプルリクエストには書き込めないトークンが渡ります。
- 主張の記録を効かせるには、ジョブで先に `geas` を走らせて、渡されたコードの `.geas/<名前>.map.jsonl` を残します。
- `merge` は人が読まなくてよいという意味で、`ritsu impact` は主張を走らせません。ジョブでテストと主張も走らせ、両方が通ったらマージしてください。
- 走るのはプルリクエストの中のワークフローなので、それを変えればこの手順も変えられます。ワークフローの変更は `other_files` に数えますが、リポジトリの設定でも `.github/` の変更に人のレビューを求めてください。
- `--lang ja` で報告が日本語になります。
