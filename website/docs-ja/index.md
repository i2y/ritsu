# ritsu

**システムが守るべき決まりを小さな言語で書き、どんな入力にも答えが一つに決まるか、決まりどうしが食い違っていないかを確かめる。確かめた決まりから、コード、人が読むページ、テストに使う入力と答えの組やシナリオを生成できる。**

コーディングエージェントは、人が一行ずつ読めるより速く、多くのコードを書くようになりました。そこで問いがはっきりします。誰が、あるいは何がコードを書いても、コードが実現すべきものは何か。サービスのあいだと中の契約。業務の規則。暦と期日。帳簿と、その上限と下限。ワークフローの骨組み。だれが何をしてよいか。要件がどこから来て、いま何がそれを満たしているか。

ritsu は、そのそれぞれに小さな言語を一つずつ与えます。どれも、人が読めるファイルです。各言語は、書いたことを確かめられるところまで確かめます。試しに選んだ例ではなく、すべての入力、すべての日、すべての道筋についてです。コードが要るところではコードを生成し、中身を理解したい人のためのページを作ります。八つが一つの処理系を共有しているので、証明が言語の境目で切れません。ワークフローは呼ぶ規則の前提を知り、規則は暦がとりうる日を知り、帳簿は規則が返しうる額を知っています。

つなぎのコードはエージェントが書き、コードが実現すべきものは ritsu が持ちます。

[ブラウザで試す](playground.md){ .md-button .md-button--primary }
[GitHub](https://github.com/i2y/ritsu){ .md-button }

## 八つの言語

| | ファイル | 何のための言語か | 詳しくは |
|---|---|---|---|
| rulec | `.rule` | 業務の規則：運賃、手数料、資格、税 | [rulec のサイト](../rulec/ja/) |
| dandori | `.flow` | ワークフローの骨組み | [dandori のサイト](../dandori/ja/) |
| koyomi | `.cal` | 期日：締め日と支払日、営業日、法令の期間 | [README](https://github.com/i2y/ritsu/blob/main/crates/koyomi/README.ja.md) |
| chobo | `.book` | 帳簿：勘定、その上限と下限、振替、仮押さえ | [README](https://github.com/i2y/ritsu/blob/main/crates/chobo/README.ja.md) |
| geas | `.geas` | エージェントが書いたコードについての主張 | [README](https://github.com/i2y/ritsu/blob/main/crates/geas/README.ja.md) |
| yuen | `.req` | 要件の出どころと、それを満たすもの | [README](https://github.com/i2y/ritsu/blob/main/crates/yuen/README.ja.md) |
| sakai | `.ctx` | 境界づけられたコンテキストの地図 | [README](https://github.com/i2y/ritsu/blob/main/crates/sakai/README.ja.md) |
| sekisho | `.gate` | だれが何をしてよいか：役割、属性、関係。規則と日付を条件に使える | [README](https://github.com/i2y/ritsu/blob/main/crates/sekisho/README.ja.md) |

どの言語も、それだけで使えます。`.flow` を書かずに rulec だけを使うこともできます。一つのファイルに二つの言語を混ぜることはしません。運賃、暦、帳簿、ワークフローを読む人は、それぞれ違うからです。

- **rulec** では、規則を、表と、そのまわりの計算、例外、ただし書きで書きます。`rulec check` は、宣言した範囲のすべての入力に答えがちょうど一つあることを証明し、単位とお金を確かめ、Lean の証明が検査し直す証明書を書きます。検査を通った規則だけが、12 の言語の普通の関数になります。
- **dandori** では、ワークフローが何をどの順に呼ぶか、リトライとタイムアウト、呼び出しが外の案件（注文、支払い、予約）をどう動かすかを書きます。`dandori check` は、すべての道筋で、案件を手を付けないか終えるかのどちらかで残し、途中で放り出さないことを示します。同じファイルが、Temporal、Step Functions、Argo Workflows、Lambda の durable functions、pydantic-graph のコードになります。
- **koyomi** では、期日を、契約や法令の言い方のまま書きます。請求日から 30 日後、休みなら次の営業日、というふうにです。`koyomi check` は、宣言した範囲のすべての日について日付を計算し、書いた条件をその一日ずつで確かめます。
- **chobo** では、勘定と、それぞれが守る上限と下限、勘定のあいだの振替を書きます。振替、仮押さえ、期限切れをどう重ねても、勘定が上限と下限の外に出ないことを確かめ、PostgreSQL と TigerBeetle を使うクライアントを生成します。
- **geas** は、プログラムについての主張を一つずつ、動いているプログラム（コマンドライン、HTTP のサービス、ブラウザのページ）に当てて確かめ、差分を渡せば、それがどの主張にかかわるかを答えます。OpenSpec の仕様を渡せば、同じ名前の主張で確かめていないシナリオを挙げます。
- **yuen** では、要件ごとに、どこから来たか、誰が受け持つか、何が満たし何が確かめるかを書きます。どのリンクも両端のハッシュで固定されます。OpenSpec の仕様の要件も、同じように固定します。
- **sakai** では、どのコンテキストがどのファイルを持ち、誰が誰に依存してよいかを地図に書きます。コンテキストのあいだをまたぐすべての参照を、ここにあるすべての言語のファイルと、サービスが契約として持つ OpenAPI と AsyncAPI の文書と、コードについて確かめます。
- **sekisho** では、どの principal がどの resource にどの action をしてよいかを、ゲート（`.gate` のファイル）に書きます。条件には、役割、属性、関係のほかに、規則の答えと日付の答えを使えます。`sekisho check` は起こりうる組み合わせを全部たどり、どれも Cedar と同じ決まりで答えを決めます。検査を通ったゲートだけが、Cedar のスキーマとポリシーと、条件の値をサービス自身のデータから計算して Cedar に尋ねる TypeScript、Python、Go のコードになります。

## 言語が出会うところ

八つが一つの処理系を共有しているのは、ある言語が確かめたことを、別の言語が前提にできるようにするためです。たとえば返金の規則は、支払った額より多くを求める人はいない、と決めてかかっています。

```rule
rule refund_check v1
description "Whether a refund is paid at once or reviewed. A refund never asks for more than was paid, which the rule takes for granted"

enum path = at_once | review

inputs
  paid  : number  range >=0 <=10000
  asked : number  range >=0 <=10000

constraint asked <= paid

outputs
  route : path

table pick
policy unique
| asked | -> route : path |
| <=100 | at_once         |
| >100  | review          |
```

ワークフローは、客が求めた額をそのまま渡して、この規則を呼びます。

```flow
workflow refund v1
description "Pays a refund back at once or sends it to review, as the rule decides"

use rule check from "refund_check.rule"
  lambda "arn:aws:lambda:us-east-1:123456789012:function:refund-check"

inputs
  order : string
  paid  : int  range >=0 <=10000

task ask_amount(order: string) -> int range >=0 <=10000
  lambda "arn:aws:lambda:us-east-1:123456789012:function:ask-amount"
  idempotent

task pay_back(order: string, amount: int range >=0 <=10000)
  lambda "arn:aws:lambda:us-east-1:123456789012:function:pay-back"
  key

flow
  let asked = ask_amount(order: order)
  let decision = check(paid: paid, asked: asked)
  match decision.route
    at_once => pay_back(order: order, amount: asked)
    review => pass
```

どちらのファイルも、それぞれの言語の検査は通ります。合わせると通りません（この二つのファイルは、`ritsu explain E201` が出す再現です）。

```console
$ ritsu check . --lang ja
ok refund_check.rule
refund.flow: 検査を通りました
エラー[ritsu E201]: refund.flow:21:1: 規則 check を呼ぶところで、前提 `asked <= paid` を破る値を渡すことがあります
    21 |   let decision = check(paid: paid, asked: asked)
  = `asked` は `>=0 <=10000`、`paid` は `>=0 <=10000` で、asked = 10000, paid = 0 のとき `asked <= paid` が成り立ちません
  = 規則から生成したコードは、前提を破る呼び出しを入口で受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。値の範囲は、dandori がその値を入れるすべての場所から集めたものです。
  = 値を渡す前に前提を保つよう分岐するか、範囲を狭めてください（入力やタスクの結果の `range`）。
ritsu check: ファイル 2 個（rulec 1、dandori 1）。検査を通らないもの 1 個（エラー 1 件）。言語の境目: 確かめた 1 か所、決められない 0 か所
```

言語をまたぐ検査の答えは、いつも三つのどれかです。成り立つ、成り立たない例がここにある、決められない（そのときは理由を言う）。黙って通すものはありません。たとえば次のものを確かめます。

- ワークフローが規則を呼ぶすべての場所で、規則の前提が成り立つか
- koyomi の日付がとりうる日を、規則の入力の範囲にすること（`range from koyomi`）。規則が宣言した範囲に収まるか
- 規則の出力を chobo の振替の額に渡すとき、その額で振替が拒否されうる理由
- chobo の仮押さえの有効期限と、ワークフローが営業日で数える待ちの長さ
- rulec、dandori、chobo で一つの型になったお金と単位
- sakai の地図のコンテキストが公開する操作に、それを守るゲートの action があるか。ワークフローが呼ぶ操作を、ゲートがそのワークフローに許しているか
- yuen は表、日付、主張、タスクを一つずつハッシュで固定し、sakai はすべての言語の参照を行番号つきで確かめる

## 人のためのページ

コードが実現すべきものを理解し、確かめたい人（業務の担当者、経理や法務、運用する人、コードをレビューする開発者）のためのページも、同じファイルから英語か日本語で作れます。書かれたことを読み、自分の知っていることと照らし合わせられます。

- `rulec doc`：規則の表、ただし書き、証明できたこと（Markdown か HTML）
- `dandori doc`：ワークフローの図と、走るすべてのシナリオ
- `koyomi doc`：暦を月ごとに
- `chobo doc`：帳簿、その上限と下限、振替
- `sekisho doc`：だれが何をしてよいかを action ごとの表にし、生成する Cedar と並べる
- `yuen trace`：要件を出典までさかのぼり、満たすものまでたどる

## 入れ方

ソースから入れます（新しい stable の Rust が要ります）。

```
cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

`ritsu check <ディレクトリ>` は、各言語の検査と、言語をまたぐ検査を走らせます。各言語のコマンドは `ritsu <言語> …` で呼びます（例：`ritsu rulec doc fee.rule`）。リリースは 0.23.0 から（rulec の番号の続き）で、リリースごとに macOS と Linux のアーカイブと、`.deb` と `.rpm` を[リリースのページ](https://github.com/i2y/ritsu/releases)に置いています。Homebrew なら `brew install i2y/tap/ritsu`、GitHub Actions なら `uses: i2y/ritsu@v0.23.0` で入ります。アーカイブには、`ritsu` と、言語の名前を付けたそれへのリンクが入っていて、リンクはその言語として動きます。

全体の設計は [DESIGN.md](https://github.com/i2y/ritsu/blob/main/DESIGN.md) にあります。ライセンスは MIT と Apache-2.0 のどちらかを選べます。

## AI エージェント向け

ritsu とその言語は、AI エージェントに使ってもらうためのものです。リポジトリに、九つの [Agent Skills](https://agentskills.io) を置いています。[ritsu のスキル](https://github.com/i2y/ritsu/tree/main/skills/ritsu)は二つ以上の言語を使うプロジェクトのためのもので、残りの八つは言語ごとのスキルです（一覧は [skills/README.ja.md](https://github.com/i2y/ritsu/blob/main/skills/README.ja.md) にあります）。入れ方は四つあります。

- **Claude Code**：このサイトがプラグインのマーケットプレイスを公開していて、プラグイン `ritsu` に九つが入っています。`/plugin marketplace add https://i2y.github.io/ritsu/marketplace.json` を実行してから、`/plugin install ritsu@ritsu` を実行します。Claude Code が取ってくるのは `skills/` のフォルダーだけで、リポジトリ全体はダウンロードしません。
- **どのエージェントでも、バイナリから**：`ritsu skills install` が、プロジェクトの `.claude/skills/` に書きます。`--user` を付けると `~/.claude/skills/` に、`--dir <dir>` を付けると、ほかのエージェントがスキルを読む場所に書きます。名前を挙げると（`ritsu skills install rulec dandori`）そのスキルだけを書き、`ritsu skills list` で一覧を出します。
- **手で**：`skills/` から必要なフォルダーを、`~/.claude/skills/` か、プロジェクトの `.claude/skills/` にコピーします。
- **リリースから**：`ritsu-skills-v<版>.zip` に九つのフォルダーが入っています。エージェントがスキルを読む場所に展開します。
