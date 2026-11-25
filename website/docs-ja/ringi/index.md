---
hide:
  - toc
---

# ringi

**会社の各部門が出して承認する申請を書くための、小さな言語です。一つの種類の申請について、フォーム、回る経路と各ステップで答える人、期限、終わったときに起きることを、一つの `.ringi` のファイルに書きます。ringi はそれを組織のデータと突き合わせ、申請を出せる人ひとりひとりについて確かめます。ritsu は同じファイルから、申請を受けるサーバーが使うコード、人が読むページ、試験を作ります。**

どの部門にも、承認を回す申請があります。経費や出張、休暇、購入、契約の締結、システムの権限、価格表の変更などです。種類ごとにフォームがあり、経路はフォームに書かれたことと、だれが出したかで変わります。申請者の上長が承認し、長い休暇なら部長も承認する。契約なら法務を通し、金額が大きければ取締役会に上げる、という具合です。ringi では、一つの種類の申請を一つの `.ringi` のファイル（申請の種類）に書きます。いくつかの申請で共通するもの（経路、組織の形、フォームで選ぶ一覧）は、別のファイルに書いて読み込みます。

ringi は、出された一件の申請をチケットとして持ちます。チケットが次にどうなるかは、四つのもので決まります。いまのチケット、届いたイベント（承認、差し戻し、催促の時刻、ワークフローの答えなど）、その時点の組織、そして時刻です。検査も、参照インタプリタも、シミュレーションも、ページも、生成するコードも、この一つの関数をもとにしています。

## だれがどのページを読むか

| 読む人 | 読むページ |
|---|---|
| `.ringi` を書くエージェントと、それを頼むエンジニア | [申請の種類の書き方](language.md)、続けて [検査が保証すること](checks.md) |
| 申請の種類を管理する人、申請がだれに回るかを知りたい承認者 | [申請の種類を読む](reading.md) |
| 申請を受けるサーバーを作る人、動かす人 | [生成する物とサーバー](server.md) |
| 申請が読むカレンダー、予算、ゲート、ワークフローを書く人 | [ほかの言語と組む](together.md) |

## 申請の種類の例

休暇の申請です（`crates/ringi/examples/leave.ja.ringi`）。申請者の上長が承認し、5 営業日を超える休暇は部長も承認します。決まりはどの例でも架空のものです。名前を日本語で書いた版を見せます。英語の名前で書いた版（`leave.ringi`）も同じディレクトリにあります。

<!-- crates/ringi/examples/leave.ja.ringi -->
```ringi
request 休暇(leave_ja) v1 "休暇の申請"
…
use organization 組織(org) from "shared/company.ringi"
use calendar 本社(office) from "shared/office.cal"
use book 休暇(leave) from "shared/leave.book"
use gate "shared/company.gate"
use route 標準(standard) from "shared/manager_approval.ringi"

form
  種類(kind) : choice "休暇の種類"
    有給(paid)    "有給休暇"
    病気(sick)    "病気休暇"
    特別(special) "特別休暇"
  初日(first_day)  : date "初日"  range >=2026-01-01 <=2027-12-31  open in 本社
  日数(days)       : number "営業日の日数"  range >=1 <=20
  復帰日(back_on)  : date "出社する日" = 初日 + 日数 business days in 本社
  理由(reason)     : long text "理由"  count <=200  required when 種類 is 特別
  代わり(cover)    : person "代わりに対応する人"  optional

route 標準(long: 日数 > 5)

ticket
…
  submitted by   everyone except team board
  self approval  not allowed
…
  same approver  approves once
  send back      to the requester, at most 2 times
…
  comment required  for reject, send back
  resubmit       within 5 business days
  withdraw       until it is decided
  on submitted   hold 休暇.take(request: ticket.id, attempt: ticket.attempt, person: requester.id, days: 日数)  when 種類 is 有給
  on approved    post 休暇.take(request: ticket.id, attempt: ticket.attempt)  when 種類 is 有給
  on sent back, rejected, withdrawn, expired  void 休暇.take(request: ticket.id, attempt: ticket.attempt)  when 種類 is 有給
```

`種類(kind)` のように括弧の中に書いた英語の名前は、生成するコードとサーバーの API で使われます。

経路は、いくつかの申請が共通して使うものです。各ステップでだれが答えるかは、人ではなく組織の中の関係（`manager of requester`）で書きます。そのため、組織が変わっても経路を書き直す必要はありません。

<!-- crates/ringi/examples/shared/manager_approval.ringi -->
```ringi
route manager_approval v1 "Approval by the manager"
…
inputs
  long : bool

route
  manager  manager
  head     dept_head  when long

who
  manager   = manager of requester
  dept_head = head of requester.department

every stage
  due in 2 business days at end of day
  remind after 1 business day at 09:00
  when late escalate, then expire
…
  when empty escalate, then skip
```

## 検査が言うこと

`ritsu ringi check` は、組織のデータをすべての版について読み、申請を出せる人ひとりひとりについて、各ステップにだれが答えるかを求めます。この例では、2027-04-01 からの組織で法務部の長がいなくなるので、弁護士二人の長い休暇は一つ上の人へ回ります。

```console
$ cd crates/ringi/examples
$ ritsu ringi check leave.ja.ringi --lang ja
警告[W502]: leave.ja.ringi:22:7: 2027-04-01 からの組織では、ステップ `head` は承認者がいないので、ほかへ回るか飛ばされます
    22 | route 標準(long: 日数 > 5)
  = `dept_head` はチーム legal（法務部）に当たり、その長が空いています。
  = ステップは `when empty escalate, then skip` なので、上へ回ります。
  = e0102（管理本部長）へ回る申請者 2 人: e9002（弁護士 2）、e9003（弁護士 3）。
  = 版のデータを直すか、そのままでよいかを確かめてください。
ok leave.ja.ringi: 申請の種類、フィールド 6 個、ステップ 2 個
  2026-04-01 から使えます: 組織 `shared/company.ringi` の最初の版（データ `../org`）
  終わり無く使えます: 読むデータに終わりがありません
  長くても 40 営業日で終わります（ステップに 3 × 10、申請者のもとで 2 × 5）
  出せる人: 2026-04-01 からの組織の 24 人のうち 19 人、2027-04-01 からの 23 人のうち 18 人（`everyone except team board`）
  組織の形で、ステップ（`head`）がほかへ回るか飛ばされる申請者: 2026-04-01 からの組織の 19 人のうち 9 人、2027-04-01 からの 18 人のうち 8 人
  組織の形で、期限を過ぎたステップ（`manager`、`head`）を上へ回せない申請者: 2026-04-01 からの組織の 19 人のうち 2 人、2027-04-01 からの 18 人のうち 2 人
  だれの申請の、どのステップがどこへ回るか: `ritsu ringi simulate leave.ja.ringi --every-requester`
```

警告は、そうなっていることを伝えるだけで、申請の種類は検査を通ります。ステップに人がいないときは上へ回す、と経路に書いてあるからです（`when empty escalate`）。通ったことを示す行の下には、いつから使えるか、一件のチケットが長くてどれだけかかるか、出せない人やほかへ回る人が何人いるかが並びます。`ritsu check` は同じ検査を、プロジェクトのほかの言語のファイルと一緒に回し、言語のあいだで受け渡すものも確かめます（[ほかの言語と組む](together.md)）。

## 動かしたときにすること

シナリオには、一件のチケットに届くイベントを一行ずつ書き、それぞれの後でチケットがどうなるかを書きます。`ritsu ringi run` は、各行を書かれた時刻に、組織と帳簿と一緒に参照インタプリタへ渡します。そして、チケットがだれを待っているか、帳簿に何をさせたかを表示します。

```console
$ cd crates/ringi/examples
$ ritsu ringi run leave.ja.ringi --scenario 長い有給休暇 --lang ja
2026-11-16 09:00  submit   e1203  -> manager（e1101）、期限は 2026-11-18 の終わり; hold 休暇.take 8 day
2026-11-16 17:00  approve  e1101  -> head（e3101）、期限は 2026-11-18 の終わり
2026-11-17 09:00  remind          -> head（e3101）、期限は 2026-11-18 の終わり; e3101 に催促した
2026-11-19 00:00  late            -> head（e0101）、期限は 2026-11-23 の終わり; head を e0101 へ上げた
2026-11-19 10:00  approve  e0101  -> approved; post 休暇.take
帳簿 `休暇` の終わりの残高: leave_left(e1203) 12 day、granted -20 day、taken 8 day
  仮押さえ `take`（T-1、attempt-1）: posted
シナリオ `長い有給休暇`: 書いたとおり（5 行）
```

部長が期限までに答えなかったので、共通の経路に書いたとおり、ステップは本部長へ上がりました。`ritsu ringi check` はシナリオもすべて回し、行に書いたステップや状態にチケットがならなければ、検査を落とします。

## ここから作られる物

- **人が読むページ。** `ritsu ringi doc` は、申請の種類を一枚のページに描きます。フォーム、各ステップでだれが答えるかを添えた経路、期限、チケットのふるまいが載ります。例のページは [申請の種類を読む](reading.md) にあります。
- **コードと契約。** `ritsu gen` は、チケットの機械と各申請の種類のコードを TypeScript、Python、Go、Rust で書きます。`--target requests` を付けると、画面、OpenAPI の文書、エージェントのツール、サーバーを試すベクタも書きます（[生成する物とサーバー](server.md)）。
- **動かせるサーバー。** Go のフレームワーク [ringi-server](https://github.com/i2y/ringi-server) が、人がブラウザで使うページ、API、エージェントのツールを出します。`ritsu ringi init` は、[その上のプロジェクト](server.md#ringi-server-で動かす)の最初のファイルを書きます。

[ブラウザで試すページ](../playground/#project=ringi/leave.ja)で、休暇の申請を、それが読むファイルと一緒に開けます。申請や共通の経路を書き換えると、すぐに検査の結果が変わります。「生成したコード」のタブでは `ringi api`、`ringi run`、`ringi extract`、`ringi vectors` が表示するものや書くものが、「人が読むページ」のタブでは申請のページが見られます。

## もっと読む

- [ringi の README](https://github.com/i2y/ritsu/blob/main/crates/ringi/README.ja.md)：言語のあらまし、コマンド、試験の回し方。
- [診断の一覧](https://github.com/i2y/ritsu/blob/main/crates/ringi/docs/codes.ja.md)：診断ごとに、いつ出るか、どう直すか、それを出す最小のファイル（`ritsu ringi explain` でも表示できます）。
- [DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/ringi/DESIGN.md)：申請をイベントで進むチケットとして扱う理由、ringi がほかの言語へ下ろすもの、サーバーの受け持ちがどこから始まるか。
- [スキル](https://github.com/i2y/ritsu/blob/main/skills/ringi/SKILL.md)：申請の種類を書いたり直したりするエージェントのためのものです。
- [ringi-server のサイト](https://i2y.github.io/ringi-server/ja/)：申請のサーバーを作るチュートリアル、しくみの説明、目的別の手引き、社員に使ってもらうときの動かし方。
