# yuen 設計文書

要件や仕様の来歴を書く小さな言語。ファイルは `.req`、コマンドは `yuen`。要件ごとに、どの出典（法令の条や文書）から来たか、誰がいつなぜそう決めたか、どの成果物が満たし、どの主張が確かめているかを書く。リンクは、人が確かめたときの両端のハッシュを持つ。出典の条、要件の文、成果物の定義のどれかが変われば、そのリンクとその先のリンクに印が付き、人が確かめ直すまで `yuen check` が止める。

名前は「ゆえん」から取った。由縁（物事のいわれ、つながり）とも、所以（わけ、根拠）とも書く。要件がどこから来たかを書き、満たすものにつなぎ、根拠が変われば止める言語なので、どちらの字の意味も合う。はじめは由来（ゆらい）から取って yurai としたが、crates.io の `yurai` が近い分野の別のツール（AI のモデルの来歴を調べるもの）なので、ritsu に取り込んだあとで yuen に改めた（ritsu の DESIGN.md 2.2）。拡張子 `.req` は変えていない。

この文書は段階 A（設計）で書き、段階 B（言語の芯）で、B が作ったコマンド（`check`、`review`、`trace`、`api`、`explain`）の出力を実物に差し替えた（4.3、5.1、6.3、9 章。どれもテストが走らせて照らし合わせる）。段階 C のうち書き出しと出典のコマンドを作ったところで、12〜14 章を実物にした（13 章の PROV-N はテストが走らせて照らし合わせ、12 章の ReqIF はテストの golden から引き、14 章の `outdated` は本物の e-Gov に一度問い合わせた出力を貼った）。段階 C の残り（一式の読み込み）は、yuen が ritsu（七つの言語を一つにまとめる処理系）に取り込まれたので、子プロセスと JSON ではなく ritsu の口（ritsu の DESIGN 3.2）で作った（ritsu の PLAN の D.7）。そこで 3 章を書き直し、8 章の affected と 11 章の api を実物にした（8 章の出力はテストが走らせて照らし合わせる）。A の段階でほかのツール（rulec 0.22.1、koyomi 0.1.0、chobo 0.1.0、geas 0.0.1）の出力、xmllint と Python の `prov`・`reqif` の振る舞いを、2026-10-03 にこの機械で実際に走らせて確かめたことは、19 章にそのまま残した。要件の端のハッシュの値（1.1、4.1）は、この文書の定義どおりに組んだ使い捨ての試作（Python）で計算したもので、段階 B の yuen は同じ値を出した（PLAN B.15）。成果物の端は、ritsu の D.7 で、それぞれの言語が口で渡す定義の文になり、ハッシュを取り直した（3.2、19 章）。

2026-10-05 に、OpenSpec の仕様を、要件ごとに固定して読む出典の種類を足した（20 章。出力は実物を貼った）。

## 0. 全体像

```
.req ── 字句・構文 ── 名前（要件と版、役割、出典、範囲、成果物の名前）
                        │
                        ├── 出典 ── 自分のコピー（sources/law/…）と固定
                        │        └── 借りた出典（rulec・koyomi のファイルの固定とコピー）
                        ├── 成果物 ── ritsu の口（中のものと定義の文、規則とカレンダーの出典、geas の主張と記録）
                        │          ├── proto（ritsu の .proto の読み手で読む）
                        │          └── ファイル（バイト列のハッシュだけ）
                        │
                        ├── 来歴のグラフ ── 端の中身とハッシュ（要件の端は、リンク元のハッシュを含む）
                        │     ├── check：確かめた記録と今のハッシュを比べる、カバレッジ、範囲、循環、期間
                        │     ├── review：人が確かめたことを、両端のハッシュとして .req に書く
                        │     ├── trace：要件か成果物から、出典まで「なぜこうなっているか」をたどる
                        │     ├── affected：差分 → 主張（geas）→ 要件 → 持ち主と出典
                        │     ├── doc：コードが実現すべきものを確かめる人と、監査する人のページ（Markdown と HTML）
                        │     ├── api：グラフ全体（JSON）
                        │     └── export：ReqIF、W3C PROV（PROV-N と PROV-JSON）
                        └── source fetch | pin | outdated（通信するのは fetch と outdated だけ）
```

### 0.1 芯にする主張

要件と、その出典と、それを満たす成果物と、それを確かめる主張を、リンクでつなぐ。リンクは、人が確かめたときの両端のハッシュを持つ。どちらかの端が変われば、そのリンクに印が付き、さらにその先のリンクにも印が付く。印は、人が確かめ直して `yuen review` が新しいハッシュを書くまで消えない。

yuen が確かめるのは、つながりとハッシュと期間だけである。要件の文の意味は確かめない。要件が出典の条文から正しく読めているか、成果物が要件を本当に満たしているか、主張が要件を十分に確かめているかは、人が読んで決める。yuen がするのは、人が読んだときから何かが変わったら、それを見落とさせないことと、読むべきものを読む順に並べることである。

看板の言い方は次のとおりで、rulec（Write rules. Prove them. Compile them.）と koyomi（Write due dates. Check every day. Compile them.）にそろえて、三つの句にした。README の頭にこの句を置いている。

- **Write where each requirement comes from. Link what meets it. Stop when anything moves.**
- **要件の出どころを書く。満たすものにつなぐ。どこかが変われば止める。**

### 0.2 なぜ作るか

一式には、出典と成果物のあいだが抜けている。

- rulec と koyomi は、規則の表や日付の関数が、法令のどの条をもとにしたかを書ける（`source … = law "<ID>" asof <日付>`、`file "<パス>" sha256:…`、行ごとの `@法 第N条`）。改正があれば `source outdated` が知らせる。
- geas は、コードの差分がどの主張に関わるかを答える（`geas affected`）。
- chobo と dandori は、決まりを守る仕組みを書くが、その決まりがどこから来たかは書かない。

けれども「何を求められたのか（要件）」と「誰がいつ、なぜそう決めたのか」は、どこにも書かれていない。そのため、次の問いに答えられない。

- 法令が改正されたとき、見直すべき要件、規則、主張はどれか。
- エージェントがコードを変えたとき、それはどの要件に関わり、誰が承認すべきか。
- 監査で「この支払日の計算は、なぜこうなっているのか」と問われたとき、条文と、その読み方を決めた人と日付まで、たどって見せられるか。

**決定**：rulec にも koyomi にも足さず、別の言語にする。

理由は、扱うものが違うことにある。rulec と koyomi は、一つの決まりの中身を書いて確かめる。yuen は、中身を書かない。いくつものツールの成果物と出典のあいだのつながりを書き、そのつながりが確かめたときのままかを見る。rulec の規則一本に要件を書き込むと、二つの規則や、規則とコードにまたがる要件を書けない。dandori を rulec の外に作ったのと同じ理由である。

**費用**：構文、検査、診断、確かめ方を、もう一揃い作ることになる。書き方は一式にそろえ（0.4）、覚えることを減らす。

### 0.3 前提

- **P1**：yuen は、つながり、ハッシュ、期間だけを確かめる。要件の文の意味、成果物が要件を満たすか、主張が要件を確かめるのに足りるかは確かめない（0.1）。
- **P2**：成果物の中身は、それぞれの言語が ritsu の口（ritsu の DESIGN 3.2）で渡すものだけから読む。`.rule`・`.cal`・`.book`・`.flow`・`.geas`・`.ctx` を yuen が自分で読み解くことはしない。例外は二つで、proto（標準の形式なので、ritsu の `.proto` の読み手で読む。rulec と dandori と sakai も同じ読み手で読む）と、法令のコピー（e-Gov と eCFR が配る XML。rulec と koyomi と同じく、タグを落として本文にする）である。ほかのファイルは、バイト列のハッシュを取るだけで、中身を解釈しない。取り込む前は「CLI が出す JSON だけから読む」と決めていた（3.1）。
- **P3**：リンクの両端のハッシュを書くのは `yuen review` だけである。rulec の `source pin` がハッシュを書くのと同じ形で、`check` は何も書かない。
- **P4**：`check` は通信しない。主張も走らせない。ほかの言語は同じプロセスの中で口から読み、子プロセスで呼ばない。geas は走らせずに、記録を geas の口で読む。通信するのは `source fetch` と `source outdated` だけで、`curl` を子プロセスで呼ぶ。
- **P5**：今日の日付を検査に使わない。効力の期間は「隙間も重なりもなく並ぶか」を確かめるだけで、「今日効いている版はどれか」は計算しない。今日の日付を使うのは、`review` が確かめた日を書くとき（`--date` を省いたとき）だけである。
- **P6**：ほかの言語の振る舞いは変えない。yuen が読みたいもののうち、口が渡すようにできたものは ritsu の口に足して出す側に答えてもらい（ritsu の D.7 で、規則とカレンダーの出典を渡す `Sources` と、差分が主張に何をもたらすかを答える `Claims::affected` を足した）、まだ渡さないものは提案として 3.5 にまとめ、それまでの扱いを決めておく。
- **P7**：成果物の参照の書き方は sakai とそろえる（2 章）。

### 0.4 一式にそろえるもの

| | そろえるもの |
|---|---|
| 書き方 | キーワードは英語の一種類だけ。名前は日本語で書ける。字下げがブロックを表し、`#` から行末までがコメント。日付の区間は `..` で、両端を含む（koyomi の 1.2） |
| 出典 | `source <名前> = law [<データベース>] "<ID>" asof <日付>` と条ごとの固定の行、`source <名前> = file "…" url "…" sha256:<16 桁>`、引用は `@<出典> <条>[, <条>…]`。コピーは `sources/law/<ID>@<日付>/<要素>.xml`。`yuen source fetch \| pin \| outdated`。`check` は通信しない（rulec の §15.68・§15.76・§15.108、koyomi の 1.5） |
| ハッシュ | SHA-256 の先頭 16 桁を `sha256:` に続けて書く。SHA-256 は自前で書く |
| 診断 | koyomi と chobo の形（`error[E301]: <ファイル>:<行>:<列>: …`、原文の行、注、そうなる例）。英語が既定で、`--lang ja` か `YUEN_LANG=ja`（ritsu のどの言語も読む `RITSU_LANG=ja` でもよい）で日本語。台帳は一か所にあり、`yuen explain` が引く |
| CLI | コマンドとフラグを一枚の表に置き、知らないフラグは exit 2（rulec の §12.1）。exit code は 0・1・2 |
| 確かめ方 | 文書に載せた出力はテストが走らせて照らし合わせる。外のツールが無ければ `SKIP:` の行を出して通す。golden の取り直しは `YUEN_BLESS=1` |
| 語 | 「証明」は使わない。通ったことは「確かめた」と書く（rulec の §15.47） |

そろえないもの：生成するコード、参照インタプリタ、ベクタ。yuen は何も計算せず、何も生成しない（書き出すのは ReqIF と PROV だけである）。

### 0.5 似たもの

2026-10-03 に、それぞれのリポジトリと文書で確かめた。

**Doorstop**（v3.2、2026-07-10）。要件を一件一ファイルの YAML にして git に置く。子の項目のリンク（`links`）は、親の UID と、親のハッシュ（stamp）を持つ。stamp は、項目の UID、`text`、`ref`、`references`（と、設定で足した属性）を順に SHA-256 に入れ、URL で使える base64 にしたものである（`doorstop/core/item.py` の `stamp()` と `types.py` の `Stamp`）。親の stamp がリンクに書かれた値と違えば、そのリンクは疑わしいリンク（suspect link）になる。`doorstop clear` がリンクの stamp を今の値に書き換え、`doorstop review` が項目自身の stamp を `reviewed` に書く。疑わしいかを見るのは親と子の一段だけで、孫には及ばない。コードへのリンクは、ファイルのパスかキーワードの検索（`ref`、`references`）で書く。

**StrictDoc**（0.30.1、2026-09-16）。SDoc という形式で、`[REQUIREMENT]` に UID・TITLE・STATEMENT・RATIONALE と、`RELATIONS`（Parent、Child、File と ROLE）を書く。`[GRAMMAR]` で型の付いたフィールド（String、SingleChoice、MultipleChoice、Tag、Reference）を決められる。要件どうしの循環は検査のエラーになる。ReqIF の読み書きは P01_SDOC という形で行う。コードへのトレースは、文書で「experimental」と書かれた機能で、コードのコメントに `@relation(…)` を書く。ハッシュでリンクの古さを見る仕組みは無く、MID（機械が振る識別子）と差分の画面がある。

**TRLC と LOBSTER**（BMW。TRLC 3.1.0、2026-09-30。LOBSTER 1.0.6、2026-07-30）。TRLC は要件を型の付いたレコードとして書く言語で、`.rsl` に型（`type Requirement { description String … }`）と検査（警告・エラー・致命的の三段）を、`.trlc` にレコードを書く。LOBSTER はトレースの証拠を集めるツールで、`lobster.conf` に段（requirements、implementation、activity）と、段ごとのつながりの決まりを書く。要件の段の `requires: "Code"; requires: "Unit Test" or "Formal Proof";` は、どの要件にもコードと、単体テストか形式的な証明のどちらかが要る、という意味である。コードの側にはコメントのタグ（Python なら `# lobster-trace: something.example`）を書き、外すなら `# lobster-exclude: <理由>` と書く。ISO 26262 のための証拠を出すことを狙っている。

**OpenFastTrace**（4.10.0、2026-09-20）。仕様の項目の ID を `型~名前~版`（`dsn~cli.tracing.default-format~1`）で書き、Markdown の項目に `Needs: impl, utest`、`Covers: req~…~1` を書く。コードにはコメントのタグ `// [impl->dsn~validate-authentication-request~1]` を書く。項目の意味が変わったら、人が版の数を上げる。上げると、古い版を指すリンクが全部無効になる。ハッシュは使わない。項目 A が項目 B を覆っていて、B に欠陥があれば、A も「not ok (transitive)」になる。

**ReqIF**（OMG、1.2、2016 年 7 月）。企業の要件管理ツールのあいだで要件をやりとりする XML の形式で、`SPEC-OBJECT`（要件）、`SPEC-RELATION`（つながり）、`SPECIFICATION`（文書の木）からなる。ツールのあいだの約束（属性の名前など）は、ReqIF Implementor Forum の実装ガイド（v1.10、2024-01-26）が補っている（12 章）。

**OpenSpec**（Fission-AI の `@fission-ai/openspec` 1.14.0、2026-09-30。2026-10-05 に確かめた）。仕様から始める開発の道具で、要件とシナリオを Markdown の仕様に書き、変更の提案（`openspec/changes/<id>/` の差分）を `openspec archive` で当てる。要件から先の成果物へのリンクも、確かめた記録も持たない。yuen は、その仕様の要件を出典として読む（20 章）。

**W3C PROV**（PROV-DM と PROV-N は 2013-04-30 の勧告、PROV-JSON は 2013-04-24 のメンバー提出）。来歴を entity（もの）、activity（行い）、agent（行う人）と、そのあいだの関係（wasDerivedFrom、wasGeneratedBy、used、wasAssociatedWith など）で書く。

どれも、リンクの先は文書の項目か、コードのタグかファイルのパスである。yuen が足すものは三つある。

1. **リンクの先が、型の付いた成果物の名前である。** `rulec "rules/送料.rule" output 送料` や `chobo "在庫.book" transfer 引当` のように、その言語が名前を持つものを指す（2 章）。名前が消えれば検査が止まり、名前の変わったものがあれば見当をつける（4.5）。
2. **出典が、法令の改正までたどれる。** 出典は rulec と koyomi と同じ形でコピーを固定し、`yuen source outdated` が改正の施行日と、見直す要件を言う（14 章）。規則やカレンダーがすでに固定している条は、それを借りて読み、`.req` に二度書かせない（3.3）。
3. **コードには geas の記録で届く。** コードにタグを書かない。geas の `map` の記録と、geas が口で答える `affected` から、差分 → 主張 → 要件 → 持ち主と出典、を答える（8 章）。

Doorstop との違いは、もう一つある。Doorstop の疑わしいリンクは一段だけだが、yuen の印はその先のリンクまで及ぶ（4.3）。OpenFastTrace が、覆われる項目の欠陥を覆う項目へ伝えるのと同じ向きで、版の数を上げる代わりにハッシュで決める。

## 1. 言語

### 1.1 ファイルの形

要件と来歴を書いたファイルの例を二つ並べる。英語の例は、米国の連邦規則（eCFR の 37 CFR 1 の四つの節）を出典にし、koyomi の例 `period_of_months.cal` を満たすものとして書いた `tests/fixtures/period_of_months/period_of_months.req` の一部である（出典は yuen が保存して固定し、成果物はファイルとして読む。三つめの要件は、読み方を決めたことも書く）。

```
requirements period_of_months v1
description "Where the dates of koyomi's example period_of_months.cal could come from, if it followed the time rules of the USPTO's regulations in 37 CFR part 1. The sources are copied and pinned here, and what meets a requirement is read as a file. Written for this test material; it does not say how any rule is read"

role legal "Decides how the words of a rule are read"
role development "Writes and fixes koyomi's file"

source cfr = law ecfr "37 CFR 1" asof 2026-01-01
  "§1.6" sha256:6bcdc27c3428886c
  "§1.7" sha256:01de176ebe4740d7
  "§1.8" sha256:fa698e3ea7cb1e49
  "§1.10" sha256:5adcbc193371fd26

scope file "period_of_months.cal"

requirement date_of_receipt
  text "Correspondence received in the Patent and Trademark Office is stamped with the date of receipt"
  in force 2026-10-01..
  owner legal
  from @cfr "§1.6"
    reviewed 2026-10-03 by legal sha256:6bcdc27c3428886c -> sha256:c6a57f069e4638f2
  satisfied by file "period_of_months.cal"
    reviewed 2026-10-03 by development sha256:c6a57f069e4638f2 -> sha256:0f1a06d9b71a39f6
  not verified "That the day of receipt is not counted is checked on koyomi doc's page, by reading the rule and its restatement side by side. This material has no claim that checks it"
    approved 2026-10-03 by legal sha256:c6a57f069e4638f2

requirement timely_filing
  text "Correspondence required to be filed within a set period of time is considered timely filed if it is mailed or transmitted before the period ends, with a certificate of the date of deposit or transmission"
  in force 2026-10-01..
  owner legal
  from @cfr "§1.8", "§1.10"
    reviewed 2026-10-03 by legal sha256:fa698e3ea7cb1e49, sha256:5adcbc193371fd26 -> sha256:8d02c6a1f0ad3364
  satisfied by file "period_of_months.cal"
    reviewed 2026-10-03 by development sha256:8d02c6a1f0ad3364 -> sha256:0f1a06d9b71a39f6
  not verified "A function that writes the rule's cases as they are, held against every origin of 2000 to 2027 and every month count of 1 to 12, was checked in koyomi's design stage. This material has no claim that checks it"
    approved 2026-10-03 by legal sha256:8d02c6a1f0ad3364
```

日本語の例は、koyomi の例 `civil_code_period_end.ja.cal`（民法 140〜143 条を文字どおりに書いた期間の計算。取り込む前の名前は `民法の期間.cal`）に、要件と来歴を書いたもの（15 章の例 `examples/civil_code_periods/civil_code_periods.ja.req` の全部）である。

```
requirements 民法の期間 v1
description "koyomi の例「civil_code_period_end.ja.cal」の日付と条件が、民法のどの条から来たか、142 条の読み方を誰が決めたかを書いた例。例として書いたもので、法令の読み方を示すものではない"

role 法務 "条文の読み方を決める"
role 開発 "koyomi のファイルを書いて直す"

source 民法 = koyomi "civil_code_period_end.ja.cal" source 民法

scope koyomi "civil_code_period_end.ja.cal" date

requirement 起算日(first_day)
  text "日、週、月又は年によって期間を定めたときは、期間の初日は、算入しない。ただし、その期間が午前零時から始まるときは、この限りでない"
  in force 2026-10-01..
  owner 法務
  from @民法 第140条
    reviewed 2026-10-04 by 法務 sha256:e880059021fbb67d -> sha256:a9ebc73907faddc8
  satisfied by koyomi "civil_code_period_end.ja.cal" date 起算日
    reviewed 2026-10-04 by 開発 sha256:a9ebc73907faddc8 -> sha256:15aa6c91d6aaae80
  not verified "初日を算入しないことは、koyomi doc のページで、条文と言い直しを見比べて確かめる。この例に、それを確かめる条件は無い"
    approved 2026-10-04 by 法務 sha256:a9ebc73907faddc8

requirement 満了日(last_day)
  text "月によって期間を定めたときは、最後の月においてその起算日に応当する日の前日の終わりに満了する。応当する日がないときは、その月の末日の終わりに満了する"
  in force 2026-10-01..
  owner 法務
  from @民法 第141条, 第143条
    reviewed 2026-10-04 by 法務 sha256:0575c131b9f08063, sha256:6950bdfb988439b6 -> sha256:465b83ed8c251406
  satisfied by koyomi "civil_code_period_end.ja.cal" date 満了日
    reviewed 2026-10-04 by 開発 sha256:465b83ed8c251406 -> sha256:23441fd408f07548
  not verified "条文の場合分けをそのまま書いた関数と、起点 2000〜2027 年のすべての日と月数 1〜12 で一致することを、koyomi の設計の段階の試作が確かめた。この例の中に、それを確かめる条件は無い"
    approved 2026-10-04 by 法務 sha256:465b83ed8c251406

requirement 満了日_142条(last_day_142)
  text "期間の末日が日曜日、国民の祝日に関する法律に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する"
  in force 2026-10-01..
  owner 法務
  from @民法 第142条
    reviewed 2026-10-04 by 法務 sha256:fc8c35a0769d3b35 -> sha256:d4f2d2a67322df17
  decided 2026-10-03 by 法務 "「その翌日」は文字どおり末日の翌日と読み、翌日も休みでもそれ以上は動かさない。休みが明けるまで動かす読み方とは、起点 2026 年と月数 1〜12 の 4,380 通りのうち 121 通りで分かれる。この例のために決めたもので、法令の読み方を示すものではない"
  satisfied by koyomi "civil_code_period_end.ja.cal" date 満了日_142条
    reviewed 2026-10-04 by 開発 sha256:d4f2d2a67322df17 -> sha256:bba4761410179e6e
  verified by koyomi "civil_code_period_end.ja.cal" claim 142条の満了日は満了日以後
    reviewed 2026-10-04 by 開発 sha256:d4f2d2a67322df17 -> sha256:7c616f5dcc9503a8
```

`reviewed` と `approved` の行は、人が書かない。人が確かめたあとに `yuen review` が書く（4.2）。例の記録は、例を作った日（2026-10-04）に `yuen review --date 2026-10-04` で書いた。ハッシュの値のうち、出典の側（`e880059021fbb67d` など）は koyomi が固定している条のコピーのハッシュ、要件の端（`a9ebc73907faddc8` など）は 4 章の定義どおりに試作で計算し、yuen が同じ値を出したもの、成果物の側（`15aa6c91d6aaae80` など）は koyomi が渡す日付と条件の定義の文のハッシュ（3.2）である。成果物の側は ritsu の D.7 で、同じ形で書いたテストの材料 `tests/fixtures/koyomi` から取り直した（取り込む前は、日付も条件もファイル全体を端にするつもりで、`民法の期間.cal` のバイト列のハッシュ `c9b94eecde23e6b5` を書いていた）。

ファイルの節は、見出し、`description`、`role`、`source`、`scope`、`requirement` の順に並ぶ（違えば E004）。`description` は一度まで。`role`・`source`・`scope`・`requirement` はいくつでも書ける。字下げは、要件の中の行と、出典の固定の行と、確かめた記録の行を表す（スペースで書き、タブは E005）。見出しの `v1` は人のための目印で、yuen は、`api` と書き出しにそのまま載せることにだけ使う。省けない（rulec と koyomi と同じ。無ければ E002）。

一つのプロジェクトは、いくつもの `.req` からなってよい。`yuen check <パス>...` に渡したファイル（ディレクトリなら下の `.req` の全部。名前が `.` で始まるディレクトリと、`target`、`node_modules` の下は見ない）が一つのプロジェクトで、要件と役割の名前は、プロジェクトの中で一つでなければならない（E007）。出典の名前は、ファイルごとに分かれる（ファイルの中の引用は、そのファイルの `source` を指す）。

### 1.2 キーワードと名前

**決定**：キーワードは英語の一種類だけ。名前（ファイルの見出し、要件、役割、出典）は日本語で書ける。要件には、ASCII の別名を丸括弧で付ける（`満了日_142条(last_day_142)`）。

| 位置 | 語 |
|---|---|
| 行頭 | `requirements` `description` `role` `source` `scope` `requirement` |
| 出典 | `law` `file` `url` `asof` `sha256:`、データベースの語 `egov` `ecfr`、借りた出典の `source` |
| 要件の中 | `text` `in force` `owner` `replaces` `from` `decided` `by` `satisfied by` `verified by` `not satisfied` `not verified` |
| 確かめた記録 | `reviewed` `approved` `by` `->` |
| 成果物の名前 | ツール名（`rulec` `dandori` `koyomi` `chobo` `geas` `proto` `openapi` `asyncapi` `cedar` `file` `yuen` `sakai` `sekisho`）と、ツールごとの種類の語（どちらも 2.3） |
| 記号 | `@`（出典の引用）、`..`（日付の区間。両端を含む）、`,`、`->`、`#`（行末までコメント） |

名前に使えないのは、行頭の語と、要件の中の語（確かめた記録の `reviewed` と `approved` を含む）である（E002）。ツール名と種類の語は、成果物の名前を書く位置（`satisfied by` と `verified by` のあと、`scope` のあと、`source <名前> =` のあと）でだけキーワードとして読む。要件の名前を `output` にしても、ほかの位置の `output` とは読み違えない。chobo が時間の語を `expires after` のあとでだけ読むのと同じ考えである。

名前は Unicode の文字・数字・`_` からなる。数字で始まってもよいが、ASCII だけで数字から始まるもの（`30days`）は、数と語のあいだの空白の書き忘れとして E001 にする（koyomi の 1.2 と同じ）。要件の版は、名前と別名のあとに `v1`、`v2` と書く（1.5）。

**別名**：別名は `[a-z][a-z0-9_]*` で（違えば E001）、プロジェクトの中で一つ（E007）。要件の名前がもとからこの形なら、別名は要らない（koyomi の 1.2 と同じ決まり）。それ以外の要件に別名が無ければ E010。版が二つ以上ある要件は、どの版にも同じ別名を書く（違えば E007、無ければ E010）。版のブロックを一つだけ読んでも、その要件の別名が分かるようにするためである。別名を使うのは、ASCII の識別子が要る外の場所である。ReqIF の `ReqIF.ForeignID`（12 章）、PROV の識別子（13 章）、`doc` の HTML の中の飛び先、`review` と `trace` の `--requirement` で名前の代わりに打つとき。`.req` の中で要件を指すとき（`from <要件>`、`replaces <要件>`）と、ほかの言語が yuen の要件を指すとき（2.3）は、名前で書く。指す綴りは名前の一つにする。

**理由**：rulec の §1.1 と同じく、同じ意味の綴りが二つあると grep と diff の両方で困る。キーワード（ASCII）と業務の語（日本語）が見た目で分かれる。

**捨てたもの**：

- 役割と出典に別名を付けること。役割と出典は、外のツールに識別子として渡すことが無い（ReqIF の要素の識別子は、どれも yuen がハッシュから作る。12 章）。
- `.req` の中で、要件を別名でも指せるようにすること。綴りが二つになり、grep で探しにくくなる。

### 1.3 役割

```
role 法務 "条文の読み方を決める"
role 経理
```

**決定**：持ち主（`owner`）、決めた人（`decided … by`）、確かめた人（`reviewed … by`）、承認した人（`approved … by`）には、プロジェクトのどこかで宣言した役割の名前を書く。宣言の無い名前は E008。役割は人でも部署でもよい。説明は省ける。

**理由**：書き間違い（`法務部` と `法務`）を検査で止めるため。`affected` が「誰が承認すべきか」を答え（8 章）、`doc` が役割の一覧を載せ（10 章）、PROV の agent になる（13 章）のは、どれも同じ名前で数えられることに頼っている。

**捨てたもの**：役割を宣言せずに、名前を自由に書くこと（上の理由）。役割に連絡先やメールアドレスを書くこと。`.req` は git に入り、公開されることもある。連絡先は、役割の名前から引ける別の場所（チームの名簿など）に置けばよい。

### 1.4 出典

**決定**：出典の書き方は三つある（20 章で、OpenSpec の仕様を読む四つめの `openspec` を足した）。

```
source 民法 = law "129AC0000000089" asof 2026-10-01
  第140条 sha256:e880059021fbb67d
  第142条 sha256:fc8c35a0769d3b35
source osha = law ecfr "29 CFR 1910" asof 2026-01-01
  "§1910.157" sha256:c2a9ce966c7e2269
source 約款 = file "docs/約款.md" url "https://example.org/terms.md" sha256:0123456789abcdef
source 民法 = koyomi "民法の期間.cal" source 民法
```

（民法と osha の固定は、koyomi と rulec の例が実際に固定している値である。`約款` の行は形を見せるためのもので、URL とハッシュは例として置いた。）

- **`law`**：rulec の `source = law`（rulec の §15.68・§15.108）と同じ形。データベースの語を省けば e-Gov、`ecfr` なら米国の eCFR。下の行に、引いている条ごとの固定を書く。コピーは `.req` の隣の `sources/law/<ID>@<日付>/<要素>.xml`（e-Gov）か `sources/law/<title>-CFR-<part>@<日付>/<section>.xml`（eCFR）で、rulec と koyomi と同じ置き方である。版の ID は `revision.txt`。条の名前から要素の名前を作る決まりも rulec と同じ（`第143条第2項` → `MainProvision-Article_143-Paragraph_2`、`別表第一` → `AppdxTable_1`、附則も rulec の §15.71 の書き方で引ける）。
- **`file`**：文書を丸ごと固定する。パスは `.req` からの相対で、そのファイルがコピーである。パスの決まりは参照のパスと同じ（2.2。絶対パスとルートの外は E013）。`url` は `source fetch` と `source outdated` が取りに行く場所。
- **借りた出典**：`<ツール> "<パス>" source <名前>`。rulec か koyomi のファイルが宣言して固定している出典を、そのまま使う。ほかのツールと、別の `.req` からは借りられない（E012）。固定のハッシュはその言語が口（`Sources`）で渡す出典の固定から、コピーはそのファイルの隣の `sources/law/…`（その言語と同じ置き方）から読む（3.3）。

引用は、要件の `from` の行に `@<出典> <条>[, <条>…]` と書く（rulec と koyomi と同じ形）。eCFR の箇所のように名前として読めないものは `"…"` で囲む（`@osha "§1910.157"`）。`file` の出典は丸ごと引く（`@約款`）。

検査は通信しない。コピーが無ければ E101（`yuen source fetch` へ）、引いている条に固定が無ければ E102（直し方は固定の行そのもの。`yuen source pin` でもよい）、コピーが固定と違えば E103、コピーが読めなければ E104、引用の書き方が読めないか、宣言の無い出典を引いていれば E105、借りた出典をそのファイルが宣言していないか、その条を固定していなければ E106、固定だけ残っていれば W101 を出す。借りた出典の固定とコピーは、それを宣言した言語が自分の検査で守っているので、yuen はコピーを読むだけで、固定と比べない（その言語の口は、検査を通らないファイルの出典を渡さず、yuen は E203 でそう言う。3.1）。

**理由**：

- 書き方を rulec と koyomi にそろえたのは、要件の出典と規則の出典が、同じ条の同じコピーを指していると見て分かるようにするためである。三つのツールが同じ置き方でコピーを持てば、`sources/law/` の下の XML は、どのツールのものでも同じバイト列になりうる。
- 借りた出典を入れたのは、規則やカレンダーがすでに保存して固定している条を、`.req` にもう一度保存して固定させないためである（3.3）。同じ条のコピーと固定が二か所にあれば、片方だけ取り直したときに食い違う。

**捨てたもの**：

- `file` の出典の中の箇所（見出しや表）を引けるようにすること。rulec は、表（`表<n>`）だけを文書から取り出して保存し、引ける（rulec の §15.82）。要件の出典になる文書（約款、契約、仕様書）で引きたいのは、表より条や段落で、それを決まった形で取り出すには文書の形ごとの読み手が要る。いまは文書を丸ごと固定し、どの段落かは要件の文に書く。要る例が出てから考える（18 章）。
- 法令をデータベースの語なしで e-Gov 以外から読むこと。rulec と同じく、データベースは `law` の語のあとに書く。

### 1.5 要件

```
requirement 支払日(payment_day) v2
  text "20 日締め翌月 10 日払い。支払日が休みなら前の営業日に払う"
  in force 2027-04-01..
  owner 経理
  from 支払の方針
  from @祝日
  decided 2026-10-03 by 経理 "2027 年 4 月から、締めを月末から 20 日に変える"
  satisfied by koyomi "支払_20日締め翌月10日払い.cal" date 支払日
  verified by koyomi "支払_20日締め翌月10日払い.cal" claim 営業日に払う
```

（行の形を見せるための例。`支払の方針` は元になった要件、`祝日` は koyomi のカレンダーから借りた内閣府の祝日の表の出典である。15 章の例とは版と期間が違う。）

**決定**：要件は `requirement <名前>(<別名>) [v<n>]` の行で始め、ブロックの行を次の順に書く（違えば E004）。

| 行 | 意味 | 数 |
|---|---|---|
| `text "<文>"` | 要件の文。短い一文を勧めるが、長さは問わない | 一つ（無ければ E010） |
| `in force <日付>..<日付>` | 効力の期間。終わりを省けば `<日付>..`、始まりを省けば `..<日付>` | 一つまで |
| `owner <役割>` | 持ち主。要件が変わるとき、承認する役割 | 一つ（無ければ E010） |
| `replaces <要件> [v<n>]` | この要件が置き換える、別の名前の要件 | いくつでも |
| `from @<出典> <条>[, <条>…]`、`from <要件> [v<n>]` | 出どころ。出典の条か、元になった要件 | いくつでも |
| `decided <日付> by <役割> "<理由>"` | 決めたこと。誰が、いつ、なぜ | いくつでも |
| `satisfied by <成果物>`、`not satisfied "<理由>"` | 満たす成果物と、満たすものを置かない見送り | いくつでも |
| `verified by <成果物>`、`not verified "<理由>"` | 確かめる主張と、確かめるものを置かない見送り | いくつでも |

要件には、出どころが少なくとも一つ要る。`from` も `decided` も無ければ E010 にする。出典から来た要件は `from @…` で、人が決めた要件は `decided` で、出どころを言う。

**版**：同じ名前の要件を `v1`、`v2` と書き分けると、同じ要件の版になる。版が一つなら `v1` は省ける。版が二つ以上あれば、どの版にも `in force` が要り、期間は隙間も重なりもなく並ばなければならない（5.5）。版の番号は期間の順に増える。版は「いつから効くか」で分ける。文の書き直し（言い回しの修正）は同じ版のまま直し、それはハッシュが拾う（4 章）。rulec が「git はいつ書いたかで、いつ効くかではない」として版に施行日を持たせなかった（rulec の §15.71）のと同じ区別を、要件の側では版と期間で書く。

**元になった要件**：`from <要件>` は、ほかの要件から読み出した要件を書く（Doorstop の親、StrictDoc の Parent にあたる）。相手の要件に版が二つ以上あれば、どの版かを書く（無ければ E009）。循環は E405。

**置き換え**：`replaces` は、名前の違う要件を置き換えるときに書く（一つの要件を二つに分ける、二つを一つにまとめる、名前を変える）。置き換えられる要件は終わりの日を持ち、置き換える要件の期間はその翌日から始まる（5.5）。

**決めたこと**：`decided` は、要件がなぜこうなっているかの記録で、出典の読み方や、出典の無い要件の決めごとを書く。日付と役割と理由の三つが要る。理由は人が読む文で、yuen は意味を見ない。決めたことは何度でも足せる。古い決めごとを消さずに新しいものを足せば、`doc` と `trace` は日付の順に並べる。

**理由**：

- 行の順序を決めたのは、要件のブロックを、上から「何を、いつから、誰の、どこから来た、誰が決めた、何が満たす、何が確かめる」と読めるようにするためである。順序が自由だと、二つのファイルで同じ要件を見比べにくい。
- 持ち主を省けなくしたのは、`affected` が「誰が承認すべきか」に答えるためである（8 章）。

**捨てたもの**：

- 状態のフィールド（draft、approved など）。承認したかどうかは、確かめた記録と見送りの承認で表す。別の状態を手で書けば、記録と食い違う。
- 優先度やタグのような、型の付いたフィールドを利用者が足せること（StrictDoc の `[GRAMMAR]`、TRLC の型）。yuen が確かめるのはつながりとハッシュと期間だけで、ほかのフィールドを足しても、検査することが無い。要る例が出てから考える（18 章）。
- 要件の文を、改行を含む長い文章にすること。文字列は一行で書く。長い説明は `decided` の理由か、出典の側に置く。

### 1.6 リンク

```
  satisfied by koyomi "民法の期間.cal" date 満了日_142条
    reviewed 2026-10-03 by 開発 sha256:d4f2d2a67322df17 -> sha256:bba4761410179e6e
  verified by geas "greeter.geas" claim "rejects an empty name"
```

**決定**：`satisfied by` は要件を満たす成果物を、`verified by` は要件を確かめる主張を、一行に一つ書く。成果物の書き方は 2 章。同じ成果物を、いくつもの要件から指してよい。

`verified by` に書けるのは、ツールが確かめて、落ちることのあるものに限る（違えば E403）。

| 書けるもの | 意味 |
|---|---|
| `geas "<spec>" claim "<名前>"` | geas が走らせる主張 |
| `koyomi "<file>" claim <名前>` | koyomi が範囲のすべての日で確かめる条件 |
| `sekisho "<file>" expect <名前>`、`sekisho "<file>" separate <名前>` | sekisho が全部の組み合わせで確かめる期待と職務の分離（2026-10-06。3.7） |
| `rulec "<file>"`、`koyomi "<file>"`、`chobo "<file>"`、`dandori "<file>"`、`geas "<spec>"`、`sekisho "<file>"` | そのツールの `check` がそのファイルに対して確かめること（rulec なら完全性、重なり、例） |
| `file "<path>"` | テストのファイル。何が走らせるかを yuen は知らない |

`rulec "x.rule" output 送料` や `chobo "在庫.book" transfer 引当` は、満たす側には書けるが、確かめる側には書けない。出力や振替は、それ自身では何も確かめないからである。

リンクの下の行の `reviewed <日付> by <役割> <ハッシュ> -> <ハッシュ>` は、人がそのリンクを確かめた記録で、`yuen review` が書く（4.2）。記録の無いリンクは、まだ確かめていないリンクとして E301 になる。

**理由**：満たす側と確かめる側を分けたのは、LOBSTER の `requires: "Code"; requires: "Unit Test" or "Formal Proof";` と同じく、作ったことと確かめたことを別々に数えるためである。確かめる側を主張に絞ったのは、出力の一つを「確かめるもの」として書いても、何も落ちないからである。yuen は主張の中身が要件に足りるかを見ない（P1）が、確かめる力の無いものを確かめる側に書く誤りは、種類だけで止められる。

**捨てたもの**：

- リンクの種類を利用者が増やせること（refines、depends on など）。要件どうしは `from` で、成果物とは二つの種類で足りる。
- `file` を確かめる側に書けなくすること。geas も koyomi も使わないプロジェクトでは、テストのファイルしか確かめるものが無い。どのファイルを書いても通ってしまうことは、P1 のとおり yuen の外である。

### 1.7 見送り

```
  not verified "月に一度、経理が支払日を照らし合わせる"
    approved 2026-10-03 by 経理 sha256:465b83ed8c251406
```

**決定**：満たす成果物か確かめる主張を置かないと決めたら、`not satisfied "<理由>"` か `not verified "<理由>"` を書く。下の行の `approved <日付> by <役割> sha256:<要件の端>` は、承認した記録で、`yuen review` が書く。承認の記録の無い見送りは E304 になる。承認のあとで要件の端が変われば、その見送りも E304 になり、承認し直すまで止まる。

**理由**：見送りも、人の判断である。判断したときの要件のハッシュを持たせれば、要件が変わったときに、見送りが古くなったことを見落とさない。リンクと同じ扱いにした。LOBSTER の `lobster-exclude: <理由>` はコードの側に書くが、yuen では要件の側に書く。外すと決めた理由を読むのは、要件の持ち主だからである。

**捨てたもの**：理由を省ける見送り。理由の無い見送りは、カバレッジの検査を黙らせるだけになる。

### 1.8 範囲

```
scope koyomi "民法の期間.cal" date
scope rulec "rules/" output
scope rulec "rules/注文の状態.rule" enum 状態 value
scope chobo "在庫.book" transfer
scope proto "shop/v1/order.proto" service OrderService method
scope dandori "flows/"
scope file "src/"
```

**決定**：`scope` は、要件に辿れなければならない成果物の集まりを宣言する。形は `scope <ツール> "<パス>" [<種類> <名前>]... [<種類>]` で、参照（2 章）のあとに、集める種類を一つ書ける。参照のパスはディレクトリでもよい。

- 最後に種類を書けば、参照が指すものの中から、その種類の成果物を全部集める。パスがディレクトリなら、その下のそのツールのファイル（`.rule`、`.cal`、`.book`、`.geas`、`.flow`、`.proto`、`.ctx`）の全部から集める。子の種類は、親の組のすぐあとにしか書けない（`scope proto "shop/v1/order.proto" service OrderService method`。2.1）。
- 種類を書かなければ、参照が指すものそのものが範囲である。パスがディレクトリなら、その下のそのツールのファイルの全部が、ファイルの単位で範囲に入る。`scope file "<パス>"` は、そのファイルか、ディレクトリなら下のファイルの全部（`.git`、`target`、`node_modules`、`.geas` と、名前が `.` で始まるディレクトリは除く）。
- 参照の組と、最後の種類は、パスのあとの語の数で分ける。奇数個なら、最後の一つが集める種類である。
- `source` の種類と `yuen` の参照は、範囲に書けない（E012。2.3）。

範囲の成果物は、どれも、いずれかの要件のリンクから辿れなければならない（E404）。辿れるかの決まりは 5.3 にある。

**理由**：成果物の側から「この出力は、なぜあるのか」を問うには、どの成果物が問われる対象かを決めておく必要がある。全部のファイルを対象にすれば、要件と関係の無いもの（設定、生成物）まで問われる。範囲は、問われるべきものを人が宣言する。

**捨てたもの**：

- glob（`src/**/*.py`）。ディレクトリとファイルで足りる。足りない例が出てから考える。
- 範囲を宣言しないとき、リンクに出てくるファイルを範囲とみなすこと。それでは、どこからも指されていない出力が見つからない。

## 2. 成果物の参照の書き方

この章の決まりは、sakai（境界づけられたコンテキストの言語、`.ctx`）と一字も違えずにそろえる。二つの言語が同じ成果物を同じ綴りで指し、同じ JSON に直せるようにするためである。参照の書き方について決めたことは、ここに全部まとめる。決まりを試す表は、ritsu に取り込んでからは ritsu-base の `tests/fixtures/naming.tsv` 一つで、sakai もこの表で確かめる。表のどの行も右の列のとおりになることを、テストが確かめる（2.6）。参照を読む仕組みも ritsu-base のもの（`ritsu_base::naming`）で、yuen に残るのは、`.req` の字句から参照を切り出すところと、診断のコードと文（E011、E012、E013）である。

### 2.1 形

```
<ツール> "<パス>" [<種類> <名前>]...
```

```
rulec "rules/送料.rule"
rulec "rules/送料.rule" output 送料
rulec "billing/請求の要否.rule" enum 注文の状態 value 受注で取消
dandori "order.flow"
koyomi "支払条件.cal" date 支払日
koyomi "支払条件.cal" claim 受領から60日以内
chobo "在庫.book" account 在庫
chobo "在庫.book" transfer 引当
geas "greeter.geas" claim "rejects an empty name"
proto "shop/v1/order.proto" service OrderService method Create
proto "shop/v1/order.proto" message Order.Line field quantity
proto "warehouse/v1/stock.proto" enum PackingStatus value PACKING_STATUS_SHORT
openapi "api/orders.yaml" operation refundOrder
openapi "api/orders.yaml" schema Refund property amount
asyncapi "events/orders.yaml" channel orderRefunded message orderRefunded
cedar "policies/refunds.cedar" policy clerks_refund_within_their_limit
cedar "policies/shop.cedarschema" action refund_order
file "src/app.py"
yuen "民法の期間.req" requirement 満了日_142条
sakai "contexts/受注.ctx" term キャンセル
```

ツール名のあとに、パスを `"…"` で書く。そのあとに、種類と名前の組をゼロ個以上続ける。組がゼロ個ならファイルそのもの、一つならファイルの中の一つのものを指す。

組を二つ書けるのは、ツールの中でものが入れ子になっているところだけで、二つめの組は、一つめのものの中にあるものを指す。

- proto：`service S [method M]`、`message M [field f]`、`enum E [value V]`。入れ子になったメッセージや列挙は、組を足さずに、名前を `.` でつないで書く（`message Order.Line`）。
- rulec：`enum E [value V]`。
- chobo：`transfer T [operation O]`（振替の操作。2026-10-06 から）。
- openapi と asyncapi：`schema S [property P]`、`schema S [value V]`。asyncapi の `channel C [message M]`（3.6）。
- 子の種類（`method`、`field`、`value`）は、親の種類（`service`、`message`、`enum`）のすぐあとにしか書けない。子のあとには、もう組を書けない。
- ほかのツールでは、組は一つまでである。

この決まりに合わない組は E012 になる（`rulec "x.rule" value 受注` は `value` が `enum` のすぐあとにない、`proto "x.proto" service S method M method N` は子が二つある、`chobo "在庫.book" account 在庫 value X` は chobo の `account` の下に組を書けない）。

### 2.2 パス

- パスは、参照を書いたファイル（`.req`）のディレクトリからの相対で書く。rulec の `file` の出典や koyomi の `use calendar` と同じである。区切りは `/` で、`.` と `..` と空の部分（`a//b` や末尾の `/`）は、字の上で畳む（シンボリックリンクはたどらない）。
- パスは `"…"` で書く。引用符の無いパス、空のパス（`""`）、絶対パス、畳んだあとでルートの外に出るパスは、どれも E013。
- 畳んで何も残らないパス（`"."`）は、書いたファイルのディレクトリそのものを指す。ディレクトリを書けるのは `scope` だけである（2.5）。ルートそのものは、文字にするときも JSON でも `"."` と書く。
- **ルート**：最初に渡したパスの上で、`.git` を持つ一番近いディレクトリ。git を走らせず、ディレクトリを見て探す。無ければ、渡したディレクトリ（ファイルなら、それがあるディレクトリ）。`--root` で替えられる。geas の `map` の記録のルート（geas の DESIGN 7.4）と同じ決まりにした。git の差分のパスも、geas の記録のパスも、このルートからの相対なので、そのまま突き合わせられる。
- JSON で出すとき（`api`、`--format json`、書き出し）は、パスをルートからの相対にする。ルートそのものは、yuen を走らせたディレクトリから見た相対で `root` に書く（そこで走らせれば `"."`）。手元の絶対パスを出力に入れないためである。
- 人が読む文面（診断、`trace`、`review`、`source` のコマンド）でファイルの場所を言うときは、一式（koyomi・chobo・dandori・geas）と同じく、走らせたディレクトリからの相対で書く。`.req` は渡したとおりのパス（ディレクトリを渡したなら、そのパスに見つけたファイルをつないだもの）で、コピーや成果物のファイルは、走らせたディレクトリからいちばん短い相対で書く（ritsu の段階 C で直した。ルートが走らせたディレクトリより上にあるとき、前はルートまで上ってから下りる `../../crates/yuen/…` の形で書いていた）。最初に渡したパスが絶対パスなら、コピーや成果物のファイルも絶対パスで書く（ritsu の土台の `paths::Shown` の決まり。渡したとおりの形で書く）。どれも、走らせた場所からそのまま開ける。参照を文字にしたもの（2.4）だけは、どこに出してもルートからの相対のままにする。読み直すと同じ参照になり、ルートにある `.req` にそのまま貼れる形だからである。

### 2.3 ツールと種類

ツール名は、`rulec`、`dandori`、`koyomi`、`chobo`、`geas`、`proto`、`openapi`、`asyncapi`、`cedar`、`file`、`yuen`、`sakai`、`sekisho` の十三である。ほかの語は E011 にする。`openapi`、`asyncapi`、`cedar` は 2026-10-06 に足した（3.6）。`sekisho` も同じ日に足した（3.7）。

種類の語は、それぞれのツールが JSON で出す名前の種類から取り、yuen と sakai が使う種類を合わせたものにした。どちらの言語も、自分では使わない種類も参照として読み、JSON に出す。

| ツール | ファイル | 種類 | 名前を読むところ |
|---|---|---|---|
| `rulec` | `.rule` | `input`、`output`、`enum`（下に `value`）、`table`、`clause`、`define`、`derive`、`machine`、`source` | rulec の口（`Items`）。`source` は `Sources` も |
| `koyomi` | `.cal` | `input`、`date`、`claim`、`source` | koyomi の口（`Items`）。`source` は `Sources` も |
| `chobo` | `.book` | `unit`、`account`、`transfer`（下に `operation`） | chobo の口（`Items`） |
| `geas` | `.geas` | `claim` | geas の口（`Items`。spec そのものから読み、`geas map` の記録は要らない） |
| `dandori` | `.flow` | `task`、`case`、`record`（下に `field`）、`enum`（下に `value`）、`input`、`output`（ritsu の D.6 で足した。2.7） | dandori の口（`Items`。構文だけから読み、規則は要らない） |
| `proto` | `.proto` | `service`（下に `method`）、`message`（下に `field`）、`enum`（下に `value`） | yuen が ritsu の `.proto` の読み手で読む（3.4） |
| `openapi` | OpenAPI の文書（`.yaml`、`.yml`、`.json`）と、文書が `$ref` で読むその一部 | `schema`（下に `property`、`value`）、`operation`、`pointer` | yuen が ritsu の YAML と JSON の読み手で読む（3.6） |
| `asyncapi` | AsyncAPI の文書と、その一部 | `channel`（下に `message`）、`message`、`operation`、`schema`（下に `property`、`value`）、`pointer` | 同じ（3.6） |
| `cedar` | `.cedar`、`.cedarschema`、`.cedarschema.json` | `policy`、`action`、`entity` | yuen が ritsu の Cedar の読み手で読む（3.6） |
| `file` | 何でも | （無い） | — |
| `yuen` | `.req` | `requirement`、`source` | yuen 自身（ほかの言語へは `yuen api`（11 章）と yuen の口） |
| `sakai` | `.ctx` | `context`、`term` | sakai の口（`Items`） |
| `sekisho` | `.gate` | `principal`（下に `attribute`）、`resource`（下に `attribute`）、`role`、`workflow`、`enum`（下に `value`）、`action`（下に `input`、`context`）、`policy`、`expect`、`separate` | sekisho の口（`Items`。構文だけから読む。3.7） |

- 取り込む前は、名前をツールの JSON（`rulec graph` の `nodes`、`rulec api` の `python.enums`、`koyomi api`、`chobo api`、geas の記録の一行め、`sakai api`）から読むと決めていた。ritsu の D.7 から、どの言語も口の `Items`（ritsu の DESIGN 6.4）で中のものを渡し、yuen はそれを読む。その言語の名前で読むので、JSON の二つの出口（`graph` と `api`）が同じ名前を出すかを確かめる必要は無くなった。
- ディレクトリを指すツール名（`dir`）は置かない。yuen でディレクトリを書けるのは `scope` だけで、その書き方は 1.8 で決める。JSON では、ディレクトリをパスの文字列で出す（11 章の `scopes`）。
- `source` は、借りた出典の宣言（1.4）にだけ書ける。借りられるのは rulec と koyomi の出典で、yuen の出典は借りられない。出典は要件の出どころで、要件を満たすものではないので、`satisfied by`、`verified by`、`scope` に書けば E012 にする。
- `yuen` は、sakai などほかの言語が yuen の要件と出典を指すための語である。yuen の `.req` の中で別の要件を指すときは、`from <要件>` と名前だけで書く（1.5）。`satisfied by`、`verified by`、`scope` に `yuen` の参照を書けば E012 にする。
- 参照の形が正しくても、yuen は、参照が指すものがあるかを必ず確かめる。読むのに要る言語がつながっていなければ（yuen のクレートのバイナリ）、`ritsu yuen` で走らせるよう言って exit 2 にする（3.1）。確かめないまま通すことはしない。

### 2.4 名前の書き方

- 名前は、その言語の名前で書く。rulec と koyomi の別名（`alias`）では書かない。書いた名前が別名に当たれば、E202 で名前のほうを示す（`output shipping_fee` → `output 送料`。別名は rulec の `Rules` と koyomi の `Dates` の口から引く）。綴りを一つにするためである。
- 名前は、引用符の無い語か、`"…"` の文字列で書く。語は、空白、`"`、`#` を含まない一続きの文字である。頭が数字でもよく（`40営業日以内`）、`.` も含められる（`Order.Line`）。yuen の名前（1.2）より広いのは、ほかのツールの名前をそのまま書けるようにするためである。空白や `#` や `"` を含む名前（geas の主張の名前に多い）は、文字列で書く。文字列の中では `\"` と `\\` だけがエスケープで、ほかの `\` は E001 にする。全角の空白は、語を区切るのではなく E001 にする（koyomi の字句と同じ）。
- 名前は書いたとおりの文字列で比べ、Unicode の正規化はしない（chobo の 1.6 と同じ）。大文字と小文字も区別する。
- proto の名前は、そのファイルの `package` から見た名前で書く。パッケージは書かない（ファイルで決まる）。入れ子は `.` でつなぐ（2.1）。
- `table` と `clause` は、名前の付いたものだけを指せる。rulec の表は名前を省けるが、名前の無い表は指せない。
- 参照を文字にするとき（診断、`api` の `text`、`trace`）は、パスをルートからの相対に直して、いつも `"…"` で囲み（中の `"` と `\` は `\"` と `\\` にする）、名前は、語として書けるなら引用符を付けず、書けなければ `"…"` で囲む。こうして文字にしたものを読み直すと、同じ参照になる。

### 2.5 同じかどうか、含むかどうか

- **同じ**：ツール名が同じで、ルートからの相対に直したパスが同じで、種類と名前の組の並びが同じとき。
- **含む**：`<ツール> "<パス>"` は、そのファイルの中のものを全部含む。親の組（proto の `service`・`message`・`enum`、rulec の `enum`、openapi と asyncapi の `schema`、asyncapi の `channel`）は、その子を全部含む。範囲（1.8、5.3）と `affected`（8 章）は、この関係を使う。
- ディレクトリは成果物ではない。ディレクトリを書けるのは `scope` だけである。

### 2.6 JSON での形

```json
{"text":"rulec \"rules/送料.rule\" output 送料","tool":"rulec","path":"rules/送料.rule","items":[["output","送料"]]}
```

キーは `text`、`tool`、`path`、`items` の順に並べる。`path` はルートからの相対、`items` は種類と名前の組の並び（組が無ければ `[]`）、`text` は 2.4 の形で文字にしたものである。一つの参照だけを出すときは、serde_json の詰めた書き方（空白を入れず、ASCII でない文字はエスケープしない）にする。`api` のように整形した JSON の中に置くときも、キーと値は同じで、違うのは空白だけである。

ritsu-base の `tests/fixtures/naming.tsv` は、この決まりを試す表である。一行が一つの参照で、タブの左が参照（書いたファイルはルートにあるとする）、右が、その参照だけを出したときの JSON か、`ERROR: <理由>` である。`tests/names.rs` は、表のどの行についても、JSON の行では一字も違わない JSON を出すこと、エラーの行ではエラーにすることを確かめる。エラーの理由は、知らないツールとツール名を `"…"` で書いたものなら E011、種類と組（種類を `"…"` で書いた、種類のあとに名前が無い、を含む）なら E012、パス（空のパスを含む）なら E013、字句（文字列の外の全角の空白、`\"` と `\\` のほかのエスケープ）なら E001 に当たることも確かめる。表は 2026-10-03 に 36 行（JSON 21 行、エラー 15 行）になり、ritsu の D.6 で dandori の種類の行を足して 42 行（JSON 24 行、エラー 18 行）になった。2026-10-06 に、ツール名 `openapi`、`asyncapi`、`cedar` の行（JSON 15 行、エラー 6 行）と、chobo の振替の操作の行（JSON 2 行、エラー 1 行）を足し、66 行（JSON 41 行、エラー 25 行）になった。chobo に入れ子ができたので、`account 在庫 value X` の行の理由は「`account` の下に組を書けない」に替わった。同じ日に、ツール名 `sekisho` の行（JSON 6 行、エラー 3 行。`attribute` を親のすぐあとでなく書いたもの、`policy` の下の組、`action` の下の `input` と `context` でない組）を足し、75 行（JSON 47 行、エラー 28 行）になった。

### 2.7 dandori と geas に足りなかったもの

- **dandori**：タスクや案件の一覧を JSON で出すコマンドが無く（`dandori check --format json` が出すのは診断だけ）、yuen は `.flow` を自分で読み解かない（P2）ので、取り込む前はファイルの単位でだけ指していた（`dandori "order.flow"`）。ritsu の D.6 で、dandori が中のものを口（`Items`）で渡すようになり、参照の決まりに dandori の種類（`task`、`case`、`record` と下の `field`、`enum` と下の `value`、`input`、`output`）が入った（ritsu の DESIGN 6.3）。D.7 から、yuen はこの種類をほかの言語の種類と同じに読み、タスクや案件の一つ一つが端になる（3.2）。dandori の口は構文だけから答えるので、フローが使う規則は要らない。dandori に無い種類と組の並びの誤りは E012 である。
- **geas**：走らせずに主張を並べるコマンドが無く、取り込む前は `geas map` の記録の一行めにある主張の名前で確かめ、記録が無ければ W201 を出して spec のファイルがあることだけを確かめる、と決めていた。D.7 から、geas の口（`Items` と `Claims`）が spec そのものから主張を渡すので、記録が無くても主張があるかを確かめる。W201 は退かせた（6.2）。記録は範囲の 4（5.3）と `affected`（8 章）にだけ使う。記録が spec のハッシュを持たないので、spec を書き換えたあとの古い記録かどうかが分からないことは変わらない（3.5）。

### 2.8 sakai と突き合わせたこと

2026-10-03 に sakai の設計と並べ、食い違っていたところを上のとおりに決めた。

1. 組を二つ書けるところを、ツールの入れ子に合わせて広げた（proto の `message … field …` と `enum … value …`、rulec の `enum … value …`。2.1）。
2. ツール名に `sakai` を足し、`dir` はツール名にしなかった（2.3）。
3. JSON のパスは、どちらの言語もルートからの相対にした（2.2）。
4. 種類の語は、二つの言語が使う種類の和にした（2.3）。
5. JSON は `items` の形にし、キーの順を決めた（2.6）。
6. 細かい形を表に足した（文字列の外の全角の空白、`\"` と `\\` のほかのエスケープ、種類とツール名を `"…"` で書いたもの、種類のあとに名前が無いもの、空のパス、`"."`、末尾の `/`。2.6）。末尾の `/` は畳んで取り除き、`"."` はルートを指して JSON でも `"."` にする。
7. 診断の文面のパスは、一式と同じく走らせたディレクトリからの相対で書き、ルートからの相対にするのは JSON だけにした（2.2）。

名前の書き方（2.4）と、同じかどうか・含むかどうか（2.5）は、もとから同じだった。

## 3. 一式の読み方

### 3.1 何を読むか

**決定**：成果物は、それぞれの言語が ritsu の口（ritsu の DESIGN 3.2）で渡すものから、同じプロセスの中で読む（P2）。yuen はほかの言語のクレートを知らず、口のトレイトだけを持つ。口は、yuen を走らせる側が渡す。`ritsu yuen` は全部の言語をつないで渡し、yuen のクレートのバイナリは何も渡さない（ritsu の DESIGN 2.3）。一回の実行の中では、一つのファイルを一つの口に一度だけ尋ねる。

中のもの（`Items`）は、yuen が言語ごとに尋ねるのではなく、プロジェクトの索引（ritsu の DESIGN 6.4 の `Index`）で引く。索引は、各言語の `Items` の答えをファイルごとに一度だけ尋ねて持ち、参照から中のものを引く（あれば中のもの、無ければ同じ親の同じ種類のもの、言語が答えなければその言語が言うこと）。`ritsu yuen` と `ritsu check` では、索引は ritsu-project が一つ作り、sakai や言語をまたぐ検査と分け合う。参照が指すものが無いときの E202 や、言語が答えないときの E203 は、これまでどおり yuen のコードと文で言う。引き方を索引に替えただけで、yuen の出力は変わらない（ritsu の PLAN の E.1）。

| ツール | 読む口 | 読むもの |
|---|---|---|
| rulec | `Items`（索引で）、`Sources`、`Rules` | 中のもの（参照の種類、名前、行、定義の文）。規則が保存して固定している出典（法令の ID と時点と条ごとの固定、`file` の出典のパスと url と固定）。名前の別名（E202 の注のため） |
| koyomi | `Items`（索引で）、`Sources`、`Dates` | 中のもの。日付のファイルとカレンダーのファイルが固定している出典。名前の別名 |
| chobo | `Items`（索引で） | 中のもの（単位、勘定、振替） |
| geas | `Items`（索引で）、`Claims` | spec の主張。`geas map` の記録（範囲の 4、5.3）と、差分が主張に何をもたらすか（`affected`、8 章） |
| dandori | `Items`（索引で） | 中のもの（構文だけから。規則は要らない） |
| sakai | `Items`（索引で） | コンテキストと語 |
| sekisho | `Items`（索引で） | `.gate` の中のもの（構文だけから。規則も契約も要らない。3.7） |
| proto | 口は使わない | ritsu の `.proto` の読み手（ritsu-proto）で yuen が読む（3.4） |
| openapi、asyncapi | 口は使わない | ritsu の YAML と JSON の読み手（`ritsu_base::yaml`）で yuen が読み、要素は `ritsu_base::document` で引く（3.6） |
| cedar | 口は使わない | ritsu の Cedar の読み手（`ritsu_base::cedar`）で yuen が読む（3.6） |
| file | 口は使わない | バイト列 |

- **言語がつながっていないとき**（E206）：yuen のクレートのバイナリが、ほかの言語のものを指すプロジェクトを渡されると、名前の段（6.1 の 2）のあとで、要る言語ごとに一つ、最初に指したところ（リンク、範囲、借りた出典）に E206 を出し、すべての言語をつないだ `ritsu yuen` で同じコマンドを走らせるよう注に書いて、exit 2 にする（引数の誤りや読めないファイルと同じく、検査の結果ではなく、走らせる場所の問題だからである）。`file` と `.proto` と法令のコピーだけのプロジェクトは、このバイナリでも全部読める。ritsu の段階 E の前は、コードの無い文を標準エラーに出していた。ritsu の受け取る側の三つの言語（dandori の E018、sakai の E104）と同じ形にそろえた（ritsu の DESIGN 2.3）。`source outdated` は、借りた出典の言語がつながっていなければ、同じ E206 を標準エラーに出して exit 2 にする。
- **言語が答えられないとき**（E203）：指したファイルが、その言語の検査を通らないか、読めないとき、その言語が言うこと（コード、場所、文）を注に並べる（ritsu の DESIGN 6.1）。rulec と koyomi のファイルは、`Sources` の口がそのファイルの検査を通してから答えるので、検査を通らない規則やカレンダーからは端を作らない。ほかの言語は、`Items` がそのファイルを読めなければ答えない。
- **定義の文が空のとき**：口が渡した中のものの定義の文が空なら、端にしない（空の文のハッシュを端にすると、何も見ていないものを確かめたことになる）。E203 で、その言語が定義の文を渡さないと言う。
- 取り込む前の決定は、CLI を子プロセスで呼んで JSON を読み（`rulec api`・`graph`、`koyomi api`、`chobo api`、`sakai api`、geas の記録のファイル）、ツールが exit 0 で終わらなければ E203、JSON に要るキーが無ければ E204、ツールが見つからなければ exit 2、というものだった。口で読むようになって、JSON の形を見ることも、ツールを探すこともなくなったので、E204 は退かせ（6.2）、E203 は上の意味に替えた。`YUEN_RULEC` などの環境変数も作らない（ritsu の DESIGN 8.2）。

借りた出典の固定は、規則の `Sources` が渡すものをそのまま使う。`yuen api` は、たとえば次のように出す（`tests/fixtures/rulec` の `yuen api` の `sources` を一行ずつに分けた。一つめが借りた出典、あとの二つは yuen が自分で保存したもの）。

```json
{"file": "fire_extinguishers.req", "name": "osha", "kind": "law", "db": "ecfr", "id": "29 CFR 1910", "asof": "2026-01-01", "revision": null, "borrowed": {"text": "rulec \"rules/osha_extinguisher.rule\" source osha", "tool": "rulec", "path": "rules/osha_extinguisher.rule", "items": [["source", "osha"]]}, "pins": [{"fragment": "§1910.157", "sha256": "c2a9ce966c7e2269"}]}
{"file": "印紙税.req", "name": "法", "kind": "law", "db": "egov", "id": "342AC0000000023", "asof": "2026-04-01", "revision": "342AC0000000023_20260401_507AC0000000052", "borrowed": null, "pins": [{"fragment": "別表第一", "sha256": "0ba69792e960021e"}]}
{"file": "印紙税.req", "name": "措置法", "kind": "law", "db": "egov", "id": "332AC0000000026", "asof": "2026-04-01", "revision": "332AC0000000026_20260401_508AC0000000012", "borrowed": null, "pins": [{"fragment": "第91条", "sha256": "85faf53f6f6e8196"}]}
```

### 3.2 成果物の端

リンクの先の端（成果物）は、端の中身（バイト列）と、その SHA-256 の先頭 16 桁のハッシュを持つ。人が確かめたのはこの中身で、これが変われば、リンクに印が付く。

**決定**：ファイルの中の一つのものを指したら、その言語が口（`Items`）で渡す、そのものの定義の文を端の中身にする（ritsu の DESIGN 6.4。何を定義の文にするかは、それぞれの言語が自分の DESIGN.md に書く）。ファイルそのものを指したら、ファイルのバイト列を端の中身にする。

| ツール | 指したもの | 端の中身 |
|---|---|---|
| どのツールも | ファイル | ファイルのバイト列 |
| rulec | `table`、`clause`、`define`、`derive`、`input`、`output`、`enum`、`machine`、`source` | そのものの行を `rulec fmt` が書く形にしたもの（表なら見出しから最後の行まで） |
| koyomi | `date`、`claim`、`input`、`source` | `date … =` の塊の行（操作の行を含む）、条件の行（コメントと前後の空白を除く） |
| chobo | `unit`、`account`、`transfer` と下の `operation` | 決まった形の JSON（下の段落） |
| geas | `claim` | 主張の塊の行 |
| dandori | `task`、`case`、`record`、`enum`、`input`、`output` と下の `field`、`value` | タスクや案件やレコードの宣言の塊の行（コメントと前後の空白を除き、文字列の外の続いた空白を一つにし、字下げは深さごとに空白二つに直す。dandori の DESIGN 0.3） |
| sakai | `context`、`term` | コンテキストのファイルの行、語の塊の行（コメントと前後の空白を除く） |
| sekisho | `principal`、`resource`、`role`、`workflow`、`enum`、`action`、`policy`、`expect`、`separate` と下の `attribute`、`value`、`input`、`context` | 宣言の塊の行（コメントと前後の空白を除き、文字列の外の続いた空白を一つにし、字下げは深さごとに空白二つに直す。dandori と同じ。sekisho の DESIGN 8.2） |
| proto | `service`、`method`、`message`、`field`、`enum`、`value` | 3.4 の決まった形の文 |
| openapi、asyncapi | `schema`、`property`、`value`、`operation`、`channel`、`message`、`pointer` | 3.6 の文（要素の値と、その `$ref` がたどる値） |
| cedar | `policy`、`action`、`entity` | 3.6 の文（ポリシー、宣言） |
| file | ファイル | バイト列 |
| yuen | `requirement` | 4.1 の要件の端の中身 |
| yuen | `source` | 出典の固定の並び（4.1 の `from` の行と同じ形で、条ごとに一行） |

chobo の定義の文は、取り込む前に `chobo api` の JSON から作ると決めていた形のままである。`unit` は単位から `name` と `ledger` を除いたもの、`account` は勘定から `name` と `code` を除いたもの、`transfer` は振替から `name`・`code`・`definition`・`operations` を除き、その移動が触る勘定（`name` と `code` を除いたもの）を勘定の名前をキーにした `accounts` として足したもの。`operation`（振替の操作。2026-10-06 から）は、その振替の文に、操作の名前（`operation`）と、拒否されうる理由の名前の並び（`refusals`）を足したもの（chobo の DESIGN、口の `Items`）。どれも、キーを UTF-8 のバイト列の順に並べ、二つの空白で字下げし、キーのあとを `": "`、改行を LF にして、最後に改行を一つ置いた JSON である（serde_json の `to_string_pretty` に、キーを並べた値を渡したものと同じ）。

テストの材料で取ったハッシュ（ritsu の D.7、`tests/suite.rs` が確かめる）：

- koyomi の `民法の期間.cal` の条件 `142条の満了日は満了日以後` の端の中身は、次の 1 行（改行なし、68 バイト）で、ハッシュは `7c616f5dcc9503a8`。日付 `満了日_142条` は `date 満了日_142条(last_day_142) = 満了日 … @民法 第142条` と `if closed + 1 day` の 2 行で、`bba4761410179e6e`。

  ```
  142条の満了日は満了日以後 : 満了日_142条 >= 満了日
  ```

- chobo の `refunds.ja.book` の振替 `返金` の端の中身は 1,206 バイトで、ハッシュは `84e9ce254075c697`（A の段階の試作と同じ値。中身は `tests/golden/ends/chobo-返金.json`）。押さえる期間（`pending`）も、移動の元の勘定の下限と拒否の理由（`返金できる残り` の `at least 0 refused as 返金超過`）も、この文に入る。
- 同じ帳簿の勘定 `売上` と `返金済み` は、どちらも外の勘定で単位が円なので、名前を除くと端の中身が同じになり、ハッシュはどちらも `35a4ec5a2ee5eb06` だった。

**理由**：

- 定義の文を端にすると、表や日付の関数や主張やタスクの一つ一つを追える。取り込む前は、rulec、koyomi の日付、geas、dandori、sakai の端をファイル全体にするしかなかった（JSON が、そのもの一つの定義を言っていなかった）。ファイル全体を端にすると、ファイルのどこを直しても、そのファイルを指すリンクの全部に印が付く。うるさくなる向きに外れたが、定義の文なら、変わったものを指すリンクにだけ印が付く（4.3 の例）。
- 定義の文を決めるのは、その言語である。yuen が言語の外から境目を決めると（たとえば `rulec certificate` から行の `cells` を抜き出すと）、言語の中が変わるたびに、定義が同じでも端が変わる。
- chobo の文が名前を除くのは、名前が変わっただけの成果物を見つけるためである（4.5）。ほかの言語の定義の文には、そのものの名前が入る（表の見出し、`date` の行）。名前を変えれば端も変わるので、4.5 の二つめの見つけ方を使う。

**捨てたもの**：

- `rulec certificate` から、行の `cells` と表の名前だけを抜き出して端の中身にすること。rulec の検査の証拠と定義の境目を、yuen が rulec の外から決めることになる。行の `cells` は、出力ごとにまとめた表の列の並びで書かれているので、別の表に列が足されただけで、変わっていない表の行も変わる。
- chobo の `definition`（chobo が定義から作るハッシュ）をそのままハッシュにすること。値は定義から決まるが、元の文が渡されないので、変わったときに差分を見せられない。
- 定義の文が作れないときに、空の文や、ファイル全体で代わりにすること。前者は何も見ていないものを確かめたことにし、後者は、言語が渡さなかったものを yuen が決めることになる（3.1）。

### 3.3 成果物が固定している出典（二度書かせない）

rulec と koyomi のファイルは、自分が保存した法令の条を固定している。

**決定**：yuen は、成果物のファイルが固定している条を、その言語の口（`Sources`）から読んで、来歴のグラフに足す。`.req` には書かせない。足した辺は「このファイルは、この条のこのコピーを固定している」で、次の四つに使う。

1. `trace`：成果物から出典までたどるとき、要件が言っていない条も、成果物が固定していれば見せる（9 章）。
2. `affected` と `source outdated`：条が変わるとき、それを固定している成果物も挙げる（8 章、14 章）。
3. 借りた出典：`source 民法 = koyomi "民法の期間.cal" source 民法` と書けば、要件の `from @民法 第142条` は、koyomi のファイルの固定とコピーをそのまま使う（1.4）。
4. 食い違いの検査（E107）：要件が `from @… 第N条` で引いている条を、その要件を満たす成果物のファイルも固定しているとき、二つのコピーの本文（タグを落としたもの）を比べる。成果物がその条を固定しているのに、どのコピーも要件のコピーと本文が違えば、E107 で本文の差分を見せる。要件が読んだ条文と、規則が保存した条文が違う、ということで、どちらかが古い。

食い違いの検査で、固定のハッシュではなく本文を比べるのは、e-Gov が、改正の無い条でも XML の属性や改行を書き換えることがあるからである（rulec の §15.71）。違う日に取った二つのコピーは、本文が同じでもバイト列が違いうる。

PROV の書き出しは、この辺を `wasInfluencedBy(成果物のファイル, 出典の条, [prov:type='yuen:pins'])` として書く（13 章）。

**いまは、ファイルの単位**：`Sources` は、ファイルが固定している条を渡すが、どの表、どの行、どの日付がどの条を引いているか（`.rule` と `.cal` の行末の `@法 第N条`）は渡さない。引用は定義の文の中に書いてあるが（`date 満了日_142条(…) = 満了日 … @民法 第142条`）、yuen はそれを読み解かない（P2）。だから yuen が読めるのは「このファイルは、この条を固定している」までで、「この出力は、この条から来た」ではない。言語が行ごとの引用を口で渡すようになれば、辺を成果物の一つ一つに細かくできる（3.5）。

**理由**：同じ条のコピーと固定が `.req` と `.rule` の二か所にあれば、片方だけ取り直したときに、二つが別の条文を指したまま、どちらの検査も通る。借りればコピーは一つで、改正を取り込むのは、それを保存した言語の `source fetch` と `source pin` の一度で済む。借りずに自分で保存した場合も、E107 が二つの食い違いを止める。

**捨てたもの**：成果物の出典を `.req` にも書かせること（`satisfied by rulec "印紙税.rule"` のそばに、その規則の出典を並べる）。言語が口で渡すことを、人に二度書かせることになる。

### 3.4 proto を読む

**決定**：`.proto` は、yuen が ritsu の `.proto` の読み手（ritsu-proto。rulec、dandori、sakai も同じものを使う。ritsu の DESIGN 4.10）で読む。標準の形式で、言語の口を通すものではないからである。読むのは proto3 の `package`、`import`、`message`（入れ子も）、`enum`、`service` と `rpc`、オプションで、`import` は、buf の置き方ならモジュールの根から、そうでなければそのファイルのディレクトリから探す。見つからない `import` の型は、書いたとおりの名前で端の中身に入る。読めない `.proto` は E205。

端の中身は、コメントと空白を落とし、一つのメンバーを一行にした決まった形の文にする。

- `message` と `enum`：宣言の順のまま、フィールド（ラベル、型の完全な名前、名前、番号、オプション）を一行ずつ。
- `method`：`rpc <名前>(<stream?> <入力の完全な名前>) returns (<stream?> <出力の完全な名前>)` とオプションの行に、入力と出力からたどれる `message` と `enum` の全部の端の中身を、完全な名前の順に足したもの。
- `service`：サービスのオプションと、すべての `method` の端の中身。
- `field`：そのフィールドの行（ラベル、型の完全な名前、名前、番号、オプション）に、フィールドの型からたどれる `message` と `enum` の全部の端の中身を、完全な名前の順に足したもの。
- `value`：その値の行（名前、番号、オプション）。

**理由**：メソッドの約束には、やりとりするメッセージの形も入る。リクエストにフィールドが一つ足されれば、そのメソッドを使う成果物を確かめ直すべきである。コメントを落としたのは、説明の書き直しだけで、約束を指すリンクに印を付けないためである。

dandori の例 `fulfillment/specs/warehouse.proto`（サービス `StockService` に `Reserve` と `Release`、列挙 `Stock`）と `fulfillment.proto` を、テストの材料（`tests/fixtures/proto`）にコピーして使う。`method Reserve` の端は `cadb2fc727e9af81`、`method Release` の端は `2a0912a66889e515` で、端の中身は `tests/golden/ends/` にある。

### 3.5 一式に足りないもの（提案）

yuen は、次のものが無くても動く（上の決定のとおり）。言語が口で渡すようになれば、yuen の検査が細かくなる。どれも、渡すかどうかはその言語の側で決める。

| ツール | 足りないもの | いまの yuen | 渡すようになれば |
|---|---|---|---|
| rulec | 表・節・`define`・`derive` と行ごとの引用（`@法 第N条`） | 出典はファイルの単位（3.3） | `Items` の中のものに引用（出典と条の並び）があれば、出力や表の一つ一つに出典の辺を持てる |
| koyomi | 日付と条件とカレンダーの行ごとの引用 | 出典はファイルの単位 | 同じく、日付ごとに出典の辺を持てる |
| geas | 記録の一行めの spec のハッシュ | 記録が spec より古くても分からない（範囲の 4 と `affected` は、そのまま記録を読む） | 記録が spec より古いことが分かり、古い記録で範囲を辿らずに済む |

取り込む前は、dandori にタスクや案件の一覧を出すコマンドが無いことも、ここに挙げていた（dandori に `api` を足す、ファイルの単位で指す、yuen が `.flow` を読み解く、の三つから、二つめで作ると決めていた）。ritsu の D.6 で dandori の口（`Items`）ができ、D.7 で yuen がそれを読むようになって、タスクや案件を一つずつ指して端にできるようになった（2.7）。

### 3.6 OpenAPI と AsyncAPI の文書、Cedar のファイル（2026-10-06）

**決定**：OpenAPI と AsyncAPI の文書の要素と、手で書いた Cedar のポリシーとスキーマの宣言を、リンクの端にする。ツール名は `openapi`、`asyncapi`、`cedar` で、種類は 2.3 の表のとおりである（参照の書き方そのものは ritsu の DESIGN 6.2）。`.proto`（3.4）と同じく標準の形式なので、どの言語の口も通さず、yuen が ritsu の読み手で読む。要件を一つの操作や一つのポリシーに結び付けられ、端もその要素の分だけになるので、同じ文書のほかの要素を直しても、そのリンクは止まらない。

**どこを指すか**。文書の中のどこがどの参照になるかと、その逆は、ritsu-base の `document` が決める。sakai も同じ関数で引くので、二つの言語は同じ要素を同じ参照で書く。

- `schema S`：`components/schemas/S`。`property P` は、その定義（`$ref` をたどった先）の `properties` の P で、無ければ `allOf` のスキーマを順に探す。`value V` は、その `enum` の値（文字列はそのまま、数と真偽は JSON の書き方、`null`）。
- OpenAPI の `operation O`：`paths` と `webhooks` の操作のうち、`operationId` が O のもの。`operationId` の無い操作だけを、方法とパスで `"POST /orders/{orderId}/refunds"` と書く。`operationId` のある操作を方法とパスで書くと E202 で、候補に `operationId` の参照が出る（名前の書き方を一つにする。2.4）。
- AsyncAPI の `channel C`（`channels` のキー）と、その下の `message M`（チャネルの `messages` のキー）、`message M`（`components/messages` のキー）、`operation O`（`operations` のキー）。
- `pointer P`：ほかのもの（レスポンス、引数、サーバー、文書の一部のファイルのスキーマ）を、ファイルの頭からの JSON Pointer で指す。sakai が `$ref` の行き着く先や、サーバーを書くのにも使う。
- Cedar の `policy`：`@id` の値。無ければ CLI と同じく、ファイルの中の順の `policy0`、`policy1`。`action` と `entity`：スキーマで宣言した名前で、名前空間は付けない。一つのスキーマのファイルで二つの名前空間が同じ名前を宣言していれば、一つに決まらないので E202 にする。
- 読めない文書（YAML か JSON として読めない、もう一方の種類の文書）と、Cedar として読めないファイル（`.cedar`、`.cedarschema`、`.cedarschema.json` のどれでもないものも）は E205 である（6.2）。

**端の中身**：

- 文書の要素：要素の場所の行（`#<JSON Pointer>`。ほかのファイルなら `<ルートからのパス>#<JSON Pointer>`）と、値を JSON で書いたもの（マップのキーを UTF-8 のバイト列の順に並べ、字下げは空白二つ。chobo の端と同じ書き方。3.2）。そのあとに、要素の `$ref` がたどれる値の全部を、同じ形で、ファイルとポインタの順に足す。ほかのファイルへの `$ref` もたどり、例やデータの下の `$ref`、URL の `$ref`、何も指さない `$ref` はたどらない（sakai と同じ決まり。sakai の DESIGN 15.5）。OpenAPI の操作には、パスの項の `parameters` と `servers`（その操作にも効くもの）も足す。`property` の場所の行は、そのプロパティを持つスキーマ（か、それを `allOf` に持つスキーマ）が `required` に挙げていれば ` (required)` で終える。`value` の端は、`enum` の場所の行と、その値だけである。
- Cedar のポリシー：コメントを除き、`cedar format` の形（幅 80、字下げ 2）で書いたポリシー（ritsu-base の `write_policy`）。
- Cedar の `action` と `entity`：その宣言を、名前空間の中に、スキーマの人が読む形で書いたもの（ritsu-base の `write_schema`）。action の `context` と、entity の属性とタグが使う共通の型（`type`）も、その中に書く。人が読む形で書けないもの（レコードでない shape）は、JSON の形で書く。

英語の材料 `tests/fixtures/refund_contracts` の、`schema Refund property amount` の端の中身は次の 7 行で、ハッシュは `55d566e37d48e6f7` である。

```
#/components/schemas/Refund/properties/amount (required)
{
  "description": "In pence",
  "maximum": 10000,
  "minimum": 1,
  "type": "integer"
}
```

`policy clerks_refund_within_their_limit` の端は次のとおりで、ハッシュは `0c7317f0e764cd08` である。ファイルの頭のコメントは入らない。

```
@id("clerks_refund_within_their_limit")
permit (
  principal in Shop::Role::"clerk",
  action == Shop::Action::"refund_order",
  resource
)
when { context.amount <= principal.refund_limit };
```

`action refund_order` の端は、`context` が使う共通の型 `RefundContext` を含む。ハッシュは `76a958d088eb5d8d` である。

```
namespace Shop {
  type RefundContext = {
    amount: Long
  };

  action "refund_order" appliesTo {
    principal: [User],
    resource: [Order],
    context: RefundContext
  };
}
```

`Refund` の `amount` の上限を 10000 から 20000 に変えると（変異 `tests/mutants/E303_element_changed`。日本語の版は `E303_要素が変わった`）、印が付くのは、その要素を指すリンクと、その要素に `$ref` でたどり着く操作を指すリンクの二本である。同じ文書のほかの操作（`getOrder`）や、ほかの操作だけが読むスキーマを直しても、印は付かない（`tests/english_contracts.rs`）。

```
$ yuen check tests/mutants/E303_element_changed --root tests/mutants/E303_element_changed
error[E303]: tests/mutants/E303_element_changed/refunds.req:10:3: openapi "api/orders.yaml" operation refundOrder changed after payments looked at this link on 2026-10-06
    10 |   satisfied by openapi "api/orders.yaml" operation refundOrder
  what changed in openapi "api/orders.yaml" operation refundOrder:
      @@ -36,5 +36,5 @@
            "amount": {
              "description": "In pence",
      -       "maximum": 10000,
      +       "maximum": 20000,
              "minimum": 1,
              "type": "integer"
  = Once a person has looked: yuen review tests/mutants/E303_element_changed --root tests/mutants/E303_element_changed --at tests/mutants/E303_element_changed/refunds.req:10 --by <role>
error[E303]: tests/mutants/E303_element_changed/refunds.req:21:3: openapi "api/orders.yaml" schema Refund property amount changed after payments looked at this link on 2026-10-06
    21 |   satisfied by openapi "api/orders.yaml" schema Refund property amount
  what changed in openapi "api/orders.yaml" schema Refund property amount:
      @@ -2,5 +2,5 @@
        {
          "description": "In pence",
      -   "maximum": 10000,
      +   "maximum": 20000,
          "minimum": 1,
          "type": "integer"
  = Once a person has looked: yuen review tests/mutants/E303_element_changed --root tests/mutants/E303_element_changed --at tests/mutants/E303_element_changed/refunds.req:21 --by <role>
tests/mutants/E303_element_changed: 2 errors
```

印は、ほかの成果物と同じく E303（リンク先が変わった）である。OpenSpec の要件の固定（20.3）が要件ごとのブロックで E103 を出すのと同じく、変わった要素の分だけが止まる。

**理由**：

- 要素ごとに端を取ると、変わった要素を指すリンクにだけ印が付く。文書を丸ごと `file` で指せば、文書のどこを直しても、その文書を指すリンクの全部に印が付く（3.2 で定義の文を端にしたのと同じ理由）。
- `$ref` の先を足すのは、`.proto` のメソッドの端に、やりとりするメッセージを入れるのと同じ理由である（3.4）。操作が受け取るスキーマの上限が変われば、その操作を指すリンクは確かめ直すべきである。
- 値を YAML のテキストではなく JSON で書くのは、コメント、引用符、キーの順、ブロックかフローかの書き方を変えただけでは止めないためである。`description` などの説明は、文書の値として契約を読む人に見せるものなので、書き直せば止まる（`.proto` のコメントは値ではないので落とした。3.4）。
- Cedar のポリシーを `cedar format` の形で書くのは、コメントと改行の位置を変えただけでは止めないためである。ritsu-base の書き手は、同じポリシーを同じテキストに書く（ritsu の DESIGN 4.18）。
- Cedar の `action` の端に、principal と resource のエンティティの宣言は入れない。エンティティの属性は、そのエンティティの端である。

**捨てたもの**：

- 要素の端を、YAML のその部分のテキストにすること。コメントや字下げを直しただけで止まり、ほかのファイルへの `$ref` の先が入らない。
- 端を、ritsu-base の `openapi`（sekisho が操作を引く読み手）の型から作ること。型が持つのは、その読み手が読むもの（引数、本文の型と範囲、`security`、レスポンスの状態）だけで、ほかの変化（レスポンスの本文、説明、ヘッダー）を見落とす。人が確かめたのは文書の値である。
- Cedar の `action` と `entity` を、名前空間を付けた名前（`Shop::User`、`Shop::Action::"refund_order"`）で書くこと。一つのファイルにはふつう名前空間が一つしかなく、action の名前は引用符を二重に書くことになる。二つの名前空間に同じ名前があるときは、E202 で言う。

**確かめ方**：`tests/english_contracts.rs`（英語の材料 `refund_contracts`）と `tests/contracts.rs`（同じものを日本語の名前で書いた `contracts`）。要素の端の golden（`tests/golden/ends/` の 10 個）、要素を変えると止まり、同じ文書のほかの操作、ほかの操作だけが読むスキーマと文書の一部、コメント、キーの順では止まらないこと、Cedar のポリシーの条件を変えると止まり、コメント、改行、ほかのポリシーでは止まらないこと、action の `context` の型を変えると止まり、エンティティとほかの action では止まらないこと、E202（書き違えたとき、文書で名前が変わったとき、`operationId` のある操作を方法とパスで書いたとき、二つの名前空間）、E205、E403（文書もポリシーも何も確かめないので、`verified by` には書けない）、範囲（`scope openapi "." operation`、`scope cedar "policies" policy` など）の E404。例は `examples/refund_contracts`（英語の `refund_contracts.req` と日本語の版。15 章）で、README（英日）の「Contracts and policies」（「契約とポリシー」）の節が、上の変異にかけた出力を見せる。

**まだやらないこと**：

- Cedar の JSON の形のポリシーと、テンプレートのリンク。ritsu-base が読まない（ritsu の DESIGN 4.18）。
- `affected` は、文書と Cedar のファイルを、ほかの成果物のファイルと同じく、ファイルの単位で答える（差分が触る要素までは絞らない）。

### 3.7 sekisho の `.gate`（2026-10-06）

**決定**：sekisho の `.gate` の中のもの（ポリシー、期待、職務の分離、action、役割、型とその属性、列挙と値、ワークフロー）を、リンクの端にする。ツール名は `sekisho` で、種類は 2.3 の表のとおりである（sekisho の DESIGN 8.4）。読むのは sekisho の口 `Items` で、ほかの言語と同じく索引で引く。

```req
requirement clerks_refund
  text "A clerk who is not suspended refunds an order"
  owner payments
  satisfied by sekisho "refunds.gate" policy clerks_refund
  verified by sekisho "refunds.gate" expect clerks_who_are_not_suspended_refund
```

- 端の中身は、そのものの宣言の塊の行である（3.2）。同じ `.gate` のほかのポリシーを直しても、そのリンクは止まらない。そのポリシー（説明や条件）を直せば、その端が変わる。
- `verified by` には、期待（`expect`）と職務の分離（`separate`）と、`.gate` のファイル全体（sekisho の検査）を書ける（1.6）。期待と職務の分離は、sekisho が全部の組み合わせで確かめ、成り立たなければ検査が落ちる（sekisho の E304、E305）。ポリシーや action は、それ自身では何も確かめないので、`satisfied by` に書く（`verified by` なら E403）。
- `Items` は構文だけから読む。規則や契約が読めない `.gate` の中のものも引ける。名前の検査を通らない `.gate` かどうかは、sekisho の検査が言う。
- yuen のクレートのバイナリは sekisho を持たないので、`.gate` を指すプロジェクトは E206 で止まる（3.1。`Suite::READ` に sekisho を足した）。
- 生成した Cedar は指さない（`.gate` の中のものと一対一の生成物だから。sekisho の DESIGN 1.3）。手で書いた Cedar は、ツール名 `cedar` で指す（3.6）。

**確かめ方**：`tests/gates.rs`。英語の材料 `tests/fixtures/refund_gates/`（`refunds.gate` と `refunds.req`）と、同じものを日本語の名前で書いた `tests/fixtures/返金のゲート/`。二つの材料のどちらでも、ポリシーと期待の端が宣言の塊の行であること、ほかのポリシー（forbid）の説明、permit の行の中の空白、行末のコメントを直しても二つの端が変わらず、permit の説明を直せばその端が変わること。E011 と E403 の文と golden は、ツール名 `sekisho` と、確かめる側に書ける sekisho の種類を足して取り直した。

## 4. ハッシュと印

### 4.1 リンクの端

リンクは、リンク元からリンク先へ向く。

| リンク | リンク元 | リンク先 |
|---|---|---|
| `from @<出典> <条>[, <条>…]` | 出典の条（いくつでも） | この要件 |
| `from <要件> [v<n>]` | 元になった要件 | この要件 |
| `satisfied by <成果物>`、`verified by <成果物>` | この要件 | 成果物 |
| `not satisfied`、`not verified`（見送り） | この要件 | （無い） |

どの端も、端の中身（バイト列）と、その SHA-256 の先頭 16 桁のハッシュを持つ。

- **出典の条**：コピーのバイト列（e-Gov か eCFR が配った XML のまま）。ハッシュは、rulec と koyomi の固定と同じ値になる。
- **`file` の出典**：そのファイルのバイト列。ハッシュは固定と同じ値。
- **成果物**：3.2 の表のとおり。
- **要件**：次の形のテキスト。

```
text <要件の文>
from law <データベース> <ID> <条> sha256:<コピーのハッシュ>
from file <ルートからのパス> sha256:<ファイルのハッシュ>
from requirement <要件の名前> v<n> sha256:<その要件の端のハッシュ>
in force <期間>
```

一行めは `text` と要件の文（文字列のエスケープを戻したもの）。二行めから下は、`from` の行（引いた条ごとに一行、元になった要件ごとに一行）と、期間があれば `in force` の行を、UTF-8 のバイト列の順に並べたものである。どの行も LF で終わる。借りた出典の条も、自分で保存した条と同じ `from law …` の行になる。持ち主、決めたこと、置き換え、リンク、見送り、要件の名前と別名と版の番号は入れない。

1.1 の `起算日` の端の中身は次の 3 行で、ハッシュは `a9ebc73907faddc8` になる（試作で計算した）。

```
text 日、週、月又は年によって期間を定めたときは、期間の初日は、算入しない。ただし、その期間が午前零時から始まるときは、この限りでない
from law egov 129AC0000000089 第140条 sha256:e880059021fbb67d
in force 2026-10-01..
```

英語の例（1.1 の `period_of_months`）の `date_of_receipt` の端の中身は次の 3 行で、ハッシュは `c6a57f069e4638f2` になる。出典は eCFR で、`from law` の行にはデータベースの語 `ecfr` と、節の名前 `§1.6` が入る。

```
text Correspondence received in the Patent and Trademark Office is stamped with the date of receipt
from law ecfr 37 CFR 1 §1.6 sha256:6bcdc27c3428886c
in force 2026-10-01..
```

**決定**：要件の端の中身に、リンク元のハッシュを入れる。

こうすると、出典の条が変われば、その条を引く要件の端のハッシュが変わり、その要件から先のリンクのハッシュも食い違う。元になった要件の端が変われば、それを元にした要件の端も変わる。変わったことが、確かめた記録と今のハッシュを比べるだけで、先へ先へと伝わる。どのリンクに印が付いているかを、yuen は記録の外に覚えておかなくてよい。

期間を入れたのは、要件がいつ効くかが変われば、それを満たす成果物（期間で切り替える規則の日付の列など）も確かめ直すべきだからである。持ち主と決めたことを入れないのは、それが変わっても、求めていることは変わらないからである。名前と別名を入れないのは、名前を変えただけで成果物を確かめ直させないためである。

**捨てたもの**：

- 要件の端を文だけにし、先のリンクへの印は、上のリンクの印から計算して出すこと。この形では、上のリンクを人が確かめた瞬間に、先のリンクの印が消える。人は、要件を直さなくてよいと判断しただけで、先の成果物を一つも見ていないのに、先のリンクの記録は、出典が変わる前の日付のまま残る。要件の文が「法令の定める日数以内」のようにあいまいなら、出典が変わって成果物を直すべきなのに、誰も成果物を見ないまま検査が通る。要件の端にリンク元のハッシュを入れれば、先のリンクは一本ずつ人が確かめるまで止まり、確かめた記録の日付が、出典が変わったあとの日付になる。監査で「改正のあと、このリンクを誰かが見たか」に、記録だけで答えられる。
- 要件の端に、コピーのハッシュではなく本文のハッシュを入れること。コピーのハッシュは rulec と koyomi の固定と同じ値で、どのツールの固定とも見比べられる。`source fetch` は本文が変わらなければコピーを書き換えない（rulec の §15.71、koyomi の 9 章と同じ。14 章）ので、属性だけの書き換えで印が付くことも無い。

### 4.2 確かめた記録

```
  from @民法 第141条, 第143条
    reviewed 2026-10-03 by 法務 sha256:0575c131b9f08063, sha256:6950bdfb988439b6 -> sha256:465b83ed8c251406
  not verified "月に一度、経理が支払日を照らし合わせる"
    approved 2026-10-03 by 経理 sha256:465b83ed8c251406
```

**決定**：リンクの下の行に `reviewed <日付> by <役割> <リンク元のハッシュ>[, <…>] -> <リンク先のハッシュ>` を、見送りの下の行に `approved <日付> by <役割> sha256:<要件の端のハッシュ>` を書く。リンク元が二つ以上（一行で二つの条を引いたとき）なら、引いた順に `,` で並べる。

- 書くのは `yuen review` だけである（P3）。人は、`check` が見せる差分を読んで確かめ、そのあとで `review` を走らせる。`review` は、何を書いたかを一行ずつ言う。
- 日付は `--date <YYYY-MM-DD>`、省けばその日（その機械のタイムゾーンの日付）。役割は `--by <役割>` で渡し、宣言の無い役割なら書かない（E008）。WASI 向けに組んだ ritsu（npm のパッケージ）では、WASI の C ライブラリがタイムゾーンを知らないので、ローダーがその機械の時計から求めて渡す `RITSU_WASI_UTC_OFFSET`（UTC からのずれの秒数）を使う。渡されなければ UTC の日付になる（ritsu の DESIGN 8.8。2026-10-10）。
- 記録は一つのリンクに一つで、確かめ直せば書き換える。前の記録は git の履歴に残る。
- `review` が変えるのは記録の行だけで、ほかのバイトは、行の終わりの CR LF も含めて一字も変えない。記録の行のあとのコメントも残す。記録の無いリンクには、その下に一行を足す。足す行の字下げは、リンクの行より空白二つ深くし、行の終わりはリンクの行に合わせる。

**理由**：

- 記録に人と日付を入れたのは、監査する人が「誰が、いつ、このつながりを見たか」を、git を開かずに読めるようにするためである（`doc` と PROV にもそのまま出る）。rulec の `source pin` はハッシュだけを書くが、コピーの固定は機械の作業で、yuen の記録は人の判断である。
- ハッシュを人に書かせないのは、`source pin` と同じ理由で、人が書き間違えたハッシュは、確かめていないものを確かめたことにするからである。

**捨てたもの**：

- 記録を `.req` の外（隠したファイルやデータベース）に置くこと。要件とリンクと記録が一つのファイルにあれば、プルリクエストの差分で、何がいつ誰に確かめられたかが一緒に読める。
- 過去の記録を全部 `.req` に残すこと。ファイルが記録で埋まる。履歴は git が持っている。

### 4.3 印の付け方

**決定**：`check` は、リンクごとに、記録のハッシュと今のハッシュを比べる。

| 状態 | 診断 |
|---|---|
| 記録が無い | E301（まだ確かめていない） |
| リンク元のハッシュが違う | E302（リンク元が変わった） |
| リンク元は同じで、リンク先のハッシュが違う | E303（リンク先が変わった） |
| 見送りに承認の記録が無いか、要件の端が違う | E304 |
| どれも同じ | 印は無い |

どれもエラーで、印の付いたリンクが一本でもあれば `check` は exit 1 で終わる。診断は、何が変わったかを言い、確かめたときの中身（4.4）があれば差分を見せる。

- 出典の条が変わった：コピーの本文の差分（タグを落として、一文を一行にしたもの。rulec と koyomi の `source outdated` が比べるのと同じ形）。
- 要件の文か期間が変わった：要件の端の中身の差分。
- 要件のリンク元だけが変わった（`from` の行のハッシュだけが違う）：差分は繰り返さず、どの条か、どの要件が変わったかを言い、上の診断を先に見るよう言う。
- 成果物が変わった：端の中身の差分。ファイル全体なら、ファイルの差分。

確かめたときの中身が `reviewed/` に無くて差分や理由を言えない印には、そのすぐあとに W301 を一つ付ける。

印は、変わったもの（出典の条、要件、成果物）ごとにまとめ、出典、要件、成果物の順に並べる。まだ確かめていないリンク（E301）、承認の無い見送り（E304）、形の崩れた記録（E305）は、変わったものが無いので、最後にまとめる。まとまりの中では、リンクを `from` のつながりの順（元が先）に、同じならファイルの行の順に並べる。一本のリンクは、最初に当たったまとまりで一度だけ言う。人が上から確かめていけば、変わったものの元を確かめてから先を確かめることになる。同じ変更（同じものの、同じハッシュから同じハッシュへの変化）は、差分を一度だけ見せ、二本めからは、どの行で見せたかを言う。

**例**：1.1 の例で、koyomi の `民法の期間.cal` が固定している 142 条のコピーが変わったとする（改正を `koyomi source fetch` で取り込み、`koyomi source pin` で固定し直した）。印が付くのは三本で、142 条を引く要件 `満了日_142条` の `from`、`satisfied by`、`verified by` が E302 になる。koyomi の固定の行が書き換わって `.cal` のバイト列は変わるが、日付と条件の端はその定義の文（3.2）で、固定の行を含まないので、ほかの要件のリンクには印が付かない（`tests/suite.rs` が、借りた出典で書いたテストの材料 `tests/fixtures/koyomi` で確かめる）。取り込む前は日付の端がファイル全体だったので、同じ `.cal` を満たす側に書いたほかの要件の二本（`起算日` と `満了日` の `satisfied by`）にも E303 が付き、五本になるはずだった。

英語の材料 `tests/fixtures/period_of_months`（米国の連邦規則 37 CFR 1 の四つの節を出典にし、出典を自分で保存し、`.cal` をファイルとして指し、確かめる側を見送りにしたもの。1.1 の英語の例）で、同じ変更をしたときの実際の出力は次のとおりである。`tests/mutants/E302_article_changed` は、1.7 節のコピーの一語を変え、固定を書き直したものである。印は三本で、変わった節を引く `from` が先に、その節から来る要件の `satisfied by` と見送りが後に並ぶ。

```
$ yuen check tests/mutants/E302_article_changed
error[E302]: tests/mutants/E302_article_changed/period_of_months.req:41:3: cfr §1.7 changed after legal looked at this link on 2026-10-03
    41 |   from @cfr "§1.7"
  = cfr §1.7 is now sha256:4b9dea2a1e492f63; it was sha256:01de176ebe4740d7 when it was looked at.
  what changed in the text (the copy tests/mutants/E302_article_changed/sources/law/37-CFR-1@2026-01-01/1.7.xml):
      @@ -1,4 +1,4 @@
        § 1.7 Times for taking action; Expiration on Saturday, Sunday or Federal holiday.
      - (a) Whenever periods of time are specified in this part in days, calendar days are intended. When the day, or the last day fixed by statute or by or under this part for taking any action or paying any fee in the United States Patent and Trademark Office falls on Saturday, Sunday, or on a Federal holiday within the District of Columbia, the action may be taken, or the fee paid, on the next succeeding business day which is not a Saturday, Sunday, or a Federal holiday. See § 90.3 of this chapter for time for appeal or for commencing civil action.
      + (a) Whenever periods of time are specified in this part in days, calendar days are intended. When the day, or the last day fixed by statute or by or under this part for taking any action or paying any fee in the United States Patent and Trademark Office falls on Saturday, Sunday, or on a Federal holiday within the District of Columbia, the action may be taken, or the fee paid, on the second succeeding business day which is not a Saturday, Sunday, or a Federal holiday. See § 90.3 of this chapter for time for appeal or for commencing civil action.
        (b) If the day that is twelve months after the filing date of a provisional application under 35 U.S.C. 111(b) and § 1.53(c) falls on Saturday, Sunday, or on a Federal holiday within the District of Columbia, the period of pendency shall be extended to the next succeeding secular or business day which is not a Saturday, Sunday, or a Federal holiday.
        [65 FR 14871, Mar. 20, 2000, as amended at 78 FR 62395, Oct. 21, 2013]
  = Once a person has looked: yuen review tests/mutants/E302_article_changed --at tests/mutants/E302_article_changed/period_of_months.req:41 --by <role>
error[E302]: tests/mutants/E302_article_changed/period_of_months.req:44:3: next_business_day comes from something that changed (cfr §1.7), so this link needs a look again
    44 |   satisfied by file "period_of_months.cal"
  = development looked at this link on 2026-10-03.
  = Look at what changed first, where its own diagnostic shows it.
  = Once a person has looked: yuen review tests/mutants/E302_article_changed --at tests/mutants/E302_article_changed/period_of_months.req:44 --by <role>
error[E304]: tests/mutants/E302_article_changed/period_of_months.req:46:3: next_business_day changed, so this waiver needs approving again (the same change as at tests/mutants/E302_article_changed/period_of_months.req:44)
    46 |   not verified "koyomi's claim moved_on_or_after_last_day checks it, but this material reads the .cal as a file, so it does not name the claim"
  = legal approved this waiver on 2026-10-03.
  = Once a person has looked: yuen review tests/mutants/E302_article_changed --at tests/mutants/E302_article_changed/period_of_months.req:46 --by <role>
tests/mutants/E302_article_changed: 3 errors
```

日本語の材料 `tests/fixtures/period`（出典を自分で保存し、`.cal` をファイルとして指し、確かめる側を見送りにしたもの）で同じ変更をしたときの、実際の出力は次のとおりである。`tests/mutants/E302_条が変わった` は、その 142 条のコピーを一文字変え、固定を書き直したものである。`.cal` は変わっていないので印は三本で、条の変わった `from` が先に、その条から来る要件の `satisfied by` と見送りが後に、一つのまとまりとして並ぶ。

```
$ yuen check tests/mutants/E302_条が変わった
error[E302]: tests/mutants/E302_条が変わった/民法の期間.req:41:3: 民法 第142条 changed after 法務 looked at this link on 2026-10-03
    41 |   from @民法 第142条
  = 民法 第142条 is now sha256:54a319e4148c24c7; it was sha256:fc8c35a0769d3b35 when it was looked at.
  what changed in the text (the copy tests/mutants/E302_条が変わった/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml):
      @@ -1,2 +1,2 @@
        第百四十二条
      - 期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。
      + 期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌々日に満了する。
  = Once a person has looked: yuen review tests/mutants/E302_条が変わった --at tests/mutants/E302_条が変わった/民法の期間.req:41 --by <role>
error[E302]: tests/mutants/E302_条が変わった/民法の期間.req:44:3: 満了日_142条 comes from something that changed (民法 第142条), so this link needs a look again
    44 |   satisfied by file "民法の期間.cal"
  = 開発 looked at this link on 2026-10-03.
  = Look at what changed first, where its own diagnostic shows it.
  = Once a person has looked: yuen review tests/mutants/E302_条が変わった --at tests/mutants/E302_条が変わった/民法の期間.req:44 --by <role>
error[E304]: tests/mutants/E302_条が変わった/民法の期間.req:46:3: 満了日_142条 changed, so this waiver needs approving again (the same change as at tests/mutants/E302_条が変わった/民法の期間.req:44)
    46 |   not verified "koyomi の条件「142条の満了日は満了日以後」が確かめているが、この材料は .cal をファイルとして読むので、その条件を指さない"
  = 法務 approved this waiver on 2026-10-03.
  = Once a person has looked: yuen review tests/mutants/E302_条が変わった --at tests/mutants/E302_条が変わった/民法の期間.req:46 --by <role>
tests/mutants/E302_条が変わった: 3 errors
```

**理由**：E302 と E303 を分けたのは、直す人が違うことが多いからである。リンク元が変わったなら、要件の持ち主が要件を読み直す。リンク先だけが変わったなら、成果物を直した人が、それでも要件を満たすかを確かめる。

### 4.4 確かめたときの中身（`reviewed/`）

差分を見せるには、確かめたときの中身が要る。ハッシュだけでは、何が変わったかが分からない。

**決定**：`review` は、記録を書くとき、両端の端の中身を `.req` の隣の `reviewed/<16 桁のハッシュ>` に書く。ファイルの名前が中身のハッシュなので、`shasum -a 256 reviewed/<ハッシュ>` の先頭 16 桁はファイルの名前と同じになり、誰でも確かめられる。同じ中身は一つのファイルで足りる。

- 1 MiB を超える中身と、UTF-8 として読めない中身（画像、PDF、xlsx）は書かない。そのリンクに印が付いたときは、ハッシュと大きさが変わったことだけを言う。
- `review` は、書き終えたあと、`reviewed/` と同じディレクトリにある `.req` のどれからも指されていないファイルを `reviewed/` から消す（名前が 16 桁の 16 進数のファイルだけ）。プロジェクトに渡さなかった `.req` の記録が指す中身を、消さないためである。
- `reviewed/` は git に入れる。コピー（`sources/`）と同じく、検査の証拠である。
- 中身のファイルが無いときも、印はハッシュで付く。差分を見せられないことを注で言う（W301）。

**捨てたもの**：

- git から前の中身を取ること。yuen は git を走らせない（`check` は git が無くても同じ結果を出す）。成果物の端の中身のうち、言語が渡す定義の文（chobo の振替の JSON など）は、そもそも git に入っていない。
- 中身を持たず、ハッシュだけを持つこと（Doorstop の stamp）。それでは「疑わしい」までしか言えず、人は git の履歴を自分で探すことになる。

### 4.5 名前の変わった成果物

**決定**：リンクが指す成果物が無くなったとき（E202）、同じファイルの同じ種類のものから、今の端のハッシュが、記録のリンク先のハッシュと同じものを探す。見つかれば「名前が変わったようです」と、その名前を添える。二つ以上見つかれば、全部を候補として並べる（3.2 の chobo の外の勘定二つのように、名前を除くと同じ定義のものがある）。

端の中身に名前が入る種類（rulec、koyomi、geas、dandori、sakai の定義の文、proto の決まった形の文。3.2）では、名前を変えれば端も変わるので、ハッシュでは見つからない。同じ種類のもので、どのリンクも指していないものを候補として並べる。候補が一つで、確かめたときの中身が `reviewed/` にあれば、それから候補のいまの定義の文への差分を見せる（名前の行だけが変わっていれば、名前が変わったと読める）。chobo の文は名前を除くので、ハッシュで見つかる。

**捨てたもの**：名前の似ているもの（編集距離）を候補にすること。似た名前の別のものを、名前が変わったものと思わせる。

## 5. 検査

### 5.1 何を確かめるか

`yuen check` は次の順に確かめる。

1. 字句と構文（E001〜E006）。読んだファイルの全文から、鍵の形の値も探す（W901。警告なので止めない）。
2. 名前（E007〜E013、E403）。要件と版、役割、出典、範囲、成果物の名前のツールと種類の語とパス、確かめる側に書ける種類。
3. 出典（E101〜E106、W101）。コピーを読み、固定と比べる。借りた出典は、借りた先の言語の口（`Sources`）を読む。
4. 成果物（E201〜E203、E205、E107）。ほかの言語のものは口（`Items`）で、proto とファイルは yuen が読む。名前があるかを確かめ、端の中身を作る。要件のコピーと、それを満たす規則やカレンダーが固定しているコピーの本文を比べる（E107）。
5. 構造（E405〜E409）。要件のあいだの循環と、版の期間。
6. リンクとハッシュ（E301〜E305、W301）。確かめた記録と今のハッシュを比べる（4.3）。
7. カバレッジと範囲（E401、E402、E404、W401）。

1 と 2 でエラーがあれば、そこで止まる。名前が決まらなければ、何も比べられないからである。3 と 4 でエラーがあっても先へ進むが、読めなかった端を持つリンクは 6 で比べず、読めなかったことを一度だけ言う。5 で循環に入った要件は端のハッシュが決まらないので、その要件のリンクも 6 で比べない。どの段も、エラーが一つでもあれば exit 1 にする。

E403（確かめる側に、主張でないものを書いた）を 2 の段に置くのは、成果物を読まなくても、参照の種類だけで決まるからである。2 の段で止めれば、ツールを呼ぶ前に言える。

通れば、何を確かめたかを一行で言う。英語の材料 `tests/fixtures/period_of_months`（37 CFR 1 の四つの節を出典にした要件三つを、出典を自分で保存し、`.cal` をファイルとして指し、確かめる側を見送りにして書いたもの）では、次のとおりである。

```
$ yuen check tests/fixtures/period_of_months
tests/fixtures/period_of_months: ok — 3 requirements, whose 6 links and 3 waivers are as they were looked at; every requirement is met and checked, or waived; the file in scope traces to a requirement
```

日本語の材料 `tests/fixtures/period`（15 章の例 `民法の期間` の要件三つを、同じように書いたもの）では、次のとおりである。

```
$ yuen check tests/fixtures/period
tests/fixtures/period: ok — 3 requirements, whose 6 links and 3 waivers are as they were looked at; every requirement is met and checked, or waived; the file in scope traces to a requirement
$ yuen check tests/fixtures/period --lang ja
tests/fixtures/period: ok — 要件 3 件のリンク 6 本と見送り 3 件が、確かめたときのままです。どの要件にも、満たすものと確かめるもの（無ければ見送り）があり、範囲のファイル 1 個は、要件に辿れます。
```

### 5.2 カバレッジと見送り

**決定**：どの要件のどの版にも、`satisfied by` か `not satisfied` が少なくとも一つ、`verified by` か `not verified` が少なくとも一つ要る（無ければ E401、E402）。リンクは、まだ確かめていなくても数え（確かめていないことは E301 が言う）、見送りも、まだ承認していなくても数える（承認の無いことは E304 が言う）。同じことを二つの診断で言わないためである。見送りと、同じ側のリンクの両方があれば W401（見送りが要らない）。

効力の期間が終わった版も、ほかの版と同じく数える。今日がどの版の期間にあたるかを、yuen は計算しない（P5）。古い版をもう満たさないなら、`not satisfied` に理由を書いて承認する。rulec のように一つの規則が日付の列で新旧の両方を受け持つなら、古い版も新しい版も、同じ規則を `satisfied by` に書ける。

**理由**：版ごとに数えるのは、法令の改正のように、新しい版が効き始めたあとも、古い版の期間に起きたこと（施行前の取引）には古い版が効くからである。

### 5.3 出どころの無い成果物

**決定**：範囲（1.8）の成果物は、次のどれかなら、要件に辿れる。

1. どれかのリンク（`satisfied by` か `verified by`）が、それを指している。
2. どれかのリンクが、それを含むもの（2.5。ファイル、親の組）を指している。
3. どれかのリンクが、それに含まれるものを指している。ファイルの単位の範囲（`scope rulec "rules/"`）で、リンクがファイルの中の出力を指しているときである。ファイルの中のものが要件を満たすなら、ファイルにも、あるわけがある。
4. `file` の成果物で、geas の記録が、そのファイルを、ある主張が走らせた行を持つファイルとして記録している。その主張か、主張の spec のファイルを、どれかのリンクが指している。

どれでもなければ E404 で、その成果物を挙げる。

4 は、コードのファイルを要件につなぐためのものである。コードにタグを書かず、geas の口（`Claims`）が渡す `geas map` の記録の、主張ごとに走らせたファイル（記録の `{"claim": …, "file": …, "ran": …}` の行。geas の DESIGN 7.4）を読む。geas の記録は、どの主張がどのファイルのどの行を走らせたかを、言語ごとのカバレッジの仕組みで測ったものである。記録のパスは、記録の一行めの `root`（spec のディレクトリからの相対）からの相対なので、yuen のルートからの相対に直してから比べる。記録の無い spec があれば、範囲の `file` の成果物の E404 に、その spec に記録が無いことを注で添える（取り込む前の W201 の代わり。2.7）。

**捨てたもの**：コードに yuen のタグ（`# yuen: 要件の名前`）を書かせること（LOBSTER と OpenFastTrace の形）。タグは書いた人が付け、外し忘れても、付け忘れても検査は気づかない。geas の記録は、主張を走らせて測ったものである。

### 5.4 循環

**決定**：`from <要件>` と `replaces <要件>` を辺にした要件のグラフに循環があれば、E405 で、循環の要件をファイルの位置と一緒に順に挙げる。StrictDoc と同じく、エラーにする。

### 5.5 効力の期間

**決定**：

- 版が一つの要件は、期間を省ける。
- 版が二つ以上の要件は、どの版にも期間が要る（E408）。版を期間の始まりの順に並べたとき、版の番号もその順に増え（E408）、終わりを開けられるのは最後の版だけ、始まりを開けられるのは最初の版だけである（E408）。
- 隣り合う版は、前の版の終わりの翌日に次の版が始まる。あいだが空けば E406 で、どの版にも入らない日を挙げる。重なれば E407 で、重なる日と二つの版を挙げる。
- `replaces` で置き換えられる要件は、終わりの日を持つ（版があれば、版を書かなければ最後の版）。`replaces` を書いた版は、その翌日に始まる（E409）。置き換えは版の行に書くので、どの版から置き換えるかが、その版の期間で決まる。二つの要件が同じ要件を置き換えてもよく（分ける）、一つの要件が二つを置き換えてもよい（まとめる）。

日付は koyomi と同じく 0001-01-01〜9999-12-31 の暦日で、無い日付（2026-02-30）は E006。

**理由**：rulec は規則の版の期間を、日付の列に対する完全性（E101）と重なり（E105）で確かめる（rulec の §15.71）。要件の版は表の行ではないので、同じことを、期間の並びとして直接確かめる。

**捨てたもの**：

- 版の期間と、出典のコピーの時点（`asof`）を突き合わせること。2014 年から効く行を 2026 年のコピーから転記するのは正しいことがあり、健全に書けない（rulec の §15.71 と同じ判断）。
- 期間を日付ではなく、出典の施行日で書くこと（`in force from @民法 第142条`）。施行日は改正ごとに違い、コピーの版の情報（`revision.txt`）に頼ることになる。

## 6. 診断

### 6.1 形

koyomi と chobo の形にそろえる。

```
error[E302]: <ファイル>:<行>:<列>: <一行の見出し>
    <行> | <原文の行>
  = <注>
  <差分か、そうなる例の見出し>:
      <差分の行や、例の行>
```

一行めは、要件と成果物の言葉で完結させる。どの診断にも、そこに至る具体的なもの（変わった本文の差分、循環の要件の並び、隙間の日付、名前の候補）を添える。直し方は、書き換えたあとの形まで書く。コマンドで直すもの（`yuen review`、`yuen source pin`）は、そのコマンドの行を書く。

文面は英語が既定で、`--lang ja` か環境変数 `YUEN_LANG=ja`（無ければ `RITSU_LANG=ja`）で日本語にする。システムのロケールは見ない。日本語と英語は `tr!("…", "…")` で隣に書き、言語は描くときに渡す（koyomi と同じく、テストが同じプロセスの中で二つの言語の golden を描けるように）。日本語の文では、ASCII の名前と日本語のあいだに空白を入れる。

日本語の文の書き方は、2026-10-05 に読み直してそろえた（ほかの言語の読み直しと同じ決まり）。読む人にしてほしいことは「〜してください」と書き、「〜を直します」「〜を書きます」のように、読む人がするのか yuen がするのか分からない言い方をしない。yuen や ritsu がすることを言うときは、「yuen は〜」「ritsu は〜」と、する側を書く。`explain` の直し方の文も同じで、「〜ときも。」で切れた文は「〜ときにも出ます。」と閉じる。この文書の語「端」（リンクの両端と、そのハッシュを取る中身）は、利用者に見える文には出さない。一つの端は「リンク元」「リンク先」と言い、両方なら「両端」と言う。端の中身のことは、そのハッシュとして「要件のハッシュ」「成果物のハッシュ」と言う（`doc` のページの「要件のハッシュ：」、E203 の注と台帳、`export` の `--help`）。ほかの言語から事実を得られないことは「<言語> から〜の情報を得られません」と言い（sakai と ritsu-cross と同じ）、ritsu の部品の名前（口、読み手）は出さずに「ritsu の .proto のパーサー」「yuen 単独のバイナリ」と言う。`trace` と `affected` の一覧の行は、行の頭に「。」で始まる切れ端を作らず、ラベルどうしは「、」でつなぐ（「引く要件 …、固定している成果物 …」「2026-10-03 に 法務 が確かめた。そのあと変わっていない」）。`doc` のページは、これまでどおり常体で書く。英語の文は一字も変えていない。

`--format json` は、プロジェクトに一つの JSON を出す：`{"root", "ok", "summary", "diagnostics": [{"code", "severity", "file", "line", "col", "message", "notes", "diff", "chain", "candidates", "fix"}]}`。`diff` は差分の行（`{"op": "-" | "+" | " " | "@@", "text"}` の並び。`@@` は統一形式の塊の見出し `@@ -18,5 +18,5 @@`）、`chain` は循環や期間の並び（`{"text", "file", "line"}`）、`candidates` は名前の候補、`fix` は `.req` に貼れる書き換えたあとの行かコマンドの行（無ければ null）。キーは `--lang` に依らず英語。文字で出すときは、行を「直した行:」、コマンドを「確かめたら:」の注にする（英語は `The line, fixed:` と `Once a person has looked:`）。

exit code は 0（エラーなし。警告はあってよい）、1（エラーあり）、2（引数の誤り、読めないファイル、要るツールが見つからない、yuen 自身の不具合）。

### 6.2 台帳

番号と意味はこの表で決め、B の段階の `src/codes.rs` に移す。各コードは、いつ出るか、どう直すか、最小の再現（走る `.req` と、要るならコピーやファイル）を持ち、`yuen explain <コード>` が引く。テストが全コードの再現を走らせ、そのコードが出ることを確かめる（koyomi と同じ）。

| コード | いつ出るか |
|---|---|
| E001 | 読めない字句（知らない文字、閉じていない文字列、形の崩れたハッシュや版や別名、ASCII だけで数字から始まる名前） |
| E002 | この位置に書けない語（名前に行頭の語や要件の中の語を使った、知らない行） |
| E003 | ファイルが `requirements` の行で始まらない |
| E004 | 節や行の順序や数が違う（`description` が二つ、要件の中の行の順、`text` が二つ、一つのリンクに記録が二つ） |
| E005 | 字下げが合わない（タブ、揃っていない、記録の行がリンクの下にない） |
| E006 | 無い日付、期間の終わりが始まりより前 |
| E007 | 同じ名前を二度宣言している（版を書かない同じ名前の要件、要件の別名、一つの要件に別名が二つ、役割、同じファイルの出典、ファイルの見出し） |
| E008 | 宣言されていない名前（役割、要件） |
| E009 | 版の誤り（`v0`、同じ版が二つ、版が二つ以上ある要件で版を書かない版、版が二つ以上ある要件を版を書かずに指した、無い版を指した） |
| E010 | 要件に要るものが無い（`text`、`owner`、出どころ（`from` か `decided`）、名前が ASCII の形でない要件の別名） |
| E011 | 知らないツール名（`dir` も） |
| E012 | ツールに無い種類の語、組の並びの誤り（子の種類が親のすぐあとにない、子が二つ、入れ子の無いツールで組が二つ、file の種類、種類のあとに名前が無い）、その位置に書けない参照（`source` をリンクや範囲に書いた、`yuen` の参照をリンクや範囲に書いた、借りた出典が rulec と koyomi の `source` でない） |
| E013 | パスの誤り（引用符が無い、空、絶対パス、ルートの外） |
| E101 | 出典のコピーが無い（OpenSpec の仕様が無いときも） |
| E102 | 出典が固定されていない（引いている条や OpenSpec の要件に固定の行が無い、`file` に `sha256:` が無い） |
| E103 | コピーが固定と違う（OpenSpec の要件のブロックが固定と違うときも） |
| E104 | コピーが読めない（XML でない、e-Gov か eCFR の形でない。OpenSpec の仕様として読めない） |
| E105 | 引用が使えない（書き方が読めない、宣言の無い出典、`file` の出典に条を書いた、OpenSpec の仕様を要件なしで引いた） |
| E106 | 借りた出典が使えない（借りた先のファイルがその出典を宣言していない、その条を固定していない、ファイルが無い、コピーが読めないか固定と違う） |
| E107 | 要件と、それを満たす規則かカレンダーが、同じ条の違う本文を読んでいる |
| E108 | OpenSpec の仕様に、固定か引用が指す要件が無い（20 章） |
| W101 | 固定した条が、どの要件からも引かれていない |
| W102 | OpenSpec の仕様の要件を、プロジェクトのどの出典も固定していない（20 章） |
| E201 | 成果物のファイルが無い（範囲のパスが無い、リンクにディレクトリを書いたときも） |
| E202 | 成果物の名前が、そのファイルに無い（別名で書いた、名前が変わった。候補を添える）。Cedar のスキーマが同じ名前を二つの名前空間で宣言していて、一つに決まらない |
| E203 | 指したものの言語が、そのファイルについて答えられない（その言語の検査を通らない、読めない、定義の文を渡さない。その言語の診断を注に添える） |
| E204 | 退いたコード（ritsu 0.23.0）。ツールの JSON が知らない形のときのためのものだった |
| E205 | proto、OpenAPI と AsyncAPI の文書、Cedar のファイルが読めない（もう一方の種類の文書を指したときも。3.6） |
| E206 | 指したものの言語が、つながっていない（yuen のクレートのバイナリ。言語ごとに一つ、最初に指したところで言い、exit 2。`ritsu yuen` で走らせる） |
| W201 | 退いたコード（ritsu 0.23.0）。geas の記録が無いので主張があるかを確かめていない、と言うためのものだった |
| E301 | まだ確かめていないリンク |
| E302 | 確かめたあとで、リンク元が変わった |
| E303 | 確かめたあとで、リンク先が変わった |
| E304 | 見送りの承認が無いか、承認のあとで要件が変わった |
| E305 | 確かめた記録の誤り（ハッシュの数がリンク元の数と合わない、記録の形が崩れている、見送りの下の `reviewed` やリンクの下の `approved`） |
| W301 | 確かめたときの中身が `reviewed/` に無いので、差分を見せられない |
| E401 | 満たす成果物も、その見送りも無い |
| E402 | 確かめる主張も、その見送りも無い |
| E403 | 確かめる側に、主張でないものを書いた |
| E404 | 範囲の成果物が、どの要件にも辿れない |
| E405 | 要件のあいだに循環がある |
| E406 | 版の期間に隙間がある |
| E407 | 版の期間が重なる |
| E408 | 版の期間の書き方が足りない（期間の無い版、終わりを開けた版が最後でない、版の番号が期間の順でない） |
| E409 | 置き換える要件の期間が、置き換えられる要件の終わりの翌日から始まらない |
| W401 | 見送りと、同じ側のリンクの両方がある |
| W402 | OpenSpec の要件のシナリオに、同じ名前の主張が、要件を確かめる主張の中に無い（20 章） |
| W901 | 鍵の形の値が `.req` に書いてある（文字列でもコメントでも） |

番台は、構文と名前（E0xx）、出典（E1xx）、成果物（E2xx）、リンクとハッシュ（E3xx）、構造とカバレッジ（E4xx）で分けた。増やすときは番台の末尾に足し、番台をまたがない。9xx の番台は、ritsu のどの言語でも同じ検査を指すセキュリティの検査に使う（ritsu の DESIGN 16 章）。

**W901（2026-10-06）**：鍵の形の値（AWS のアクセスキー ID、GitHub・Slack・Stripe・OpenAI・Anthropic・Google の鍵やトークン、Slack の Incoming Webhook の URL、PEM の秘密鍵）が、`.req`（文字列でもコメントでも）に書いてある。プロジェクトの `.req` のファイルごとに、1 の段で調べる。調べ方は ritsu-base の `secrets` で、ritsu のどの言語も同じ決まりで調べる（ritsu の DESIGN 16.3）。診断には鍵の種類と接頭辞と長さだけを出し、鍵そのものは出さない。鍵のある行も引用しない。テスト用の値は、同じ行のコメントに `ritsu: test secret` と書けば言わない。

退いたコードは、台帳に残して `yuen explain` で引けるようにし、退いた理由と版を書く。番号はほかのものに使い回さない（ritsu の DESIGN 7.10、rulec の docs/compatibility.md と同じ決まり）。ritsu の D.7 で、子プロセスと JSON のために決めていた E204 と、記録で主張の名前を確かめていた W201 を退かせ、E203 は、言語がファイルについて答えられないこと（その言語の検査を通らない、読めない、定義の文を渡さない）を、その言語の診断を挙げて言う意味に替えた。E106、E107、E202、E203、E205 の再現は、ほかの言語のファイルを隣に置く `.req` で、テストはすべての言語をつないで走らせる（`ritsu yuen` と同じ）。

台帳は、コードごとに再現を二つ持つ（E206 の一つは英語で、両方の言語で同じものを見せる。退いたコードは持たない）。`yuen explain` は、`--lang en` では英語の再現（名前も英語で、法令は eCFR）を、`--lang ja` では日本語の再現を見せる。日本語の出力は変えていない。英語の出力に出る再現が日本語から英語になったのは、決めて変えたことである（英語を先にする）。テストは、どちらの再現も同じに走らせ、そのコードが出ることを確かめる（`tests/codes.rs`。42 と 41 の 83 回。20 章で E108 と W102 を足して、44 と 43 の 87 回。W901 を足して、46 と 45 の 91 回）。

### 6.3 診断の例

範囲のファイルが要件に辿れないとき（`tests/mutants/E404_file_in_scope`。範囲は `src` で、リンクが指すのは `src/pay.py` だけ）：

```
$ yuen check tests/mutants/E404_file_in_scope --root tests/mutants/E404_file_in_scope
error[E404]: tests/mutants/E404_file_in_scope/payment.req:7:1: file "src/report.py" is in scope, and no requirement leads to it
     7 | scope file "src"
  = The rest of the scope is reached: file "src/pay.py"
  = Add a requirement whose `satisfied by` or `verified by` names it, or narrow the scope.
tests/mutants/E404_file_in_scope: 1 error
```

日本語の材料では（`tests/mutants/E404_範囲のファイル`）：

```
$ yuen check tests/mutants/E404_範囲のファイル --root tests/mutants/E404_範囲のファイル
error[E404]: tests/mutants/E404_範囲のファイル/支払.req:7:1: file "src/report.py" is in scope, and no requirement leads to it
     7 | scope file "src"
  = The rest of the scope is reached: file "src/pay.py"
  = Add a requirement whose `satisfied by` or `verified by` names it, or narrow the scope.
tests/mutants/E404_範囲のファイル: 1 error
```

版の期間に隙間があるとき（`tests/mutants/E406_gap_of_one_day`）：

```
$ yuen check tests/mutants/E406_gap_of_one_day
error[E406]: tests/mutants/E406_gap_of_one_day/payment.req:18:3: 2027-04-01, between payment_day v1 and v2, falls in no version's period
    18 |   in force 2027-04-02..
  = v1 is in force `2026-01-01..2027-03-31`, and v2 `2027-04-02..`.
  in the order of the periods:
      v1 in force 2026-01-01..2027-03-31 (tests/mutants/E406_gap_of_one_day/payment.req:8)
      v2 in force 2027-04-02.. (tests/mutants/E406_gap_of_one_day/payment.req:18)
  = The line, fixed: in force 2027-04-01..
tests/mutants/E406_gap_of_one_day: 1 error
```

日本語の材料では（`tests/mutants/E406_隙間が一日`）：

```
$ yuen check tests/mutants/E406_隙間が一日 --lang ja
エラー[E406]: tests/mutants/E406_隙間が一日/支払.req:18:3: 要件「支払日」の v1 と v2 のあいだの 2027-04-01 が、どの版の期間にも入りません
    18 |   in force 2027-04-02..
  = v1 の期間は `2026-01-01..2027-03-31`、v2 の期間は `2027-04-02..` です。
  期間の順:
      v1 in force 2026-01-01..2027-03-31（tests/mutants/E406_隙間が一日/支払.req:8）
      v2 in force 2027-04-02..（tests/mutants/E406_隙間が一日/支払.req:18）
  = 直した行: in force 2027-04-01..
tests/mutants/E406_隙間が一日: エラー 1 件
```

## 7. コマンド

| コマンド | すること |
|---|---|
| `yuen check <path>... [--format json]` | 検査（5 章）。ディレクトリを渡すと、下の `.req` を全部（パスの順に）。渡したものが一つのプロジェクト |
| `yuen review <path>... (--at <file.req>:<行>)... (--requirement '<名前>[ v<n>]')... [--all] --by <役割> [--date <YYYY-MM-DD>]` | 選んだリンクと見送りのうち、印の付いているものに、確かめた記録を書く（4.2）。確かめたときの中身を `reviewed/` に書く（4.4） |
| `yuen trace <path>... (--requirement '<名前>[ v<n>]' \| --artifact '<成果物>' \| --source '@<出典> <条>') [--format json]` | 一つのものから、出典まで「なぜこうなっているか」をたどって見せる（9 章） |
| `yuen affected <path>... --diff <file\|-> [--map <spec.geas>=<記録>]... [--format json]` | 差分が触る要件と、その持ち主と出典（8 章） |
| `yuen doc <path>... [--format markdown\|html] [--out <dir>]` | コードが実現すべきものを理解し、確かめる人と、来歴を監査する人のページ（10 章）。既定は Markdown で標準出力 |
| `yuen api <path>...` | プロジェクトのグラフ全体を JSON で（11 章） |
| `yuen export reqif\|prov <path>... [--format provn\|json] [--time <RFC 3339>] [--out <file>]` | ReqIF（12 章）か W3C PROV（13 章）に書き出す。PROV の既定は PROV-N |
| `yuen source fetch\|pin\|outdated <path>...` | 出典を取ってきて保存する・固定する・元が変わったかを問う（14 章） |
| `yuen explain <コード>`、`yuen explain --all [--format markdown]` | 診断のコードを引く |

どのコマンドにも `--lang ja|en`（無ければ `YUEN_LANG`、次に `RITSU_LANG`、どちらも無ければ英語）と `--root <dir>`（2.2）を付けられる。`yuen --help`、`yuen <コマンド> --help`、`yuen --version`。

**決定**：コマンドとフラグの定義を `src/cli.rs` の一枚の表に置き、`--help` の表示と引数の読み取りが同じ表を引く（rulec の §12.1、koyomi の 5 章）。知らないフラグ、閉じた集合の外の値、値の無いフラグ、二度書いたフラグ（`--at` と `--requirement` と `--map` のように、何度でも書けると表に書いたものを除く）は exit 2 で止める。黙って無視すると、エージェントはフラグが効いたと信じて次に進むからである。

`review` の選び方：

- `--at <file.req>:<行>`：その行のリンクか見送り（記録の行の番号でもよい）。`check` の診断が出す位置をそのまま渡せる。
- `--requirement <名前>`：その要件の、印の付いたリンクと見送りの全部。名前の代わりに別名でもよい。`'支払日 v2'` のように `.req` と同じ綴りで版を選べる。
- `--all`：プロジェクトの、印の付いたリンクと見送りの全部。
- 選んだものに印が無ければ、何も書かず、そう言う（確かめた日付を、確かめ直していないのに新しくしない）。
- 端の中身を作れないリンク（3〜5 の段のエラーに関わるもの）には書かず、理由を言って exit 1。1 と 2 の段にエラーがあれば、何も書かずに exit 1。
- 書いたリンクごとに、何が変わっていたか（E301〜E304 のどれか）と、書いた記録の行を一行ずつ言う。

通信するのは `source fetch` と `source outdated` の二つだけである。`check`・`review`・`trace`・`affected`・`doc`・`api`・`export` は通信しない。テストが PATH から `curl` を外して確かめる。

## 8. affected

**決定**：`yuen affected <path>... --diff <差分> [--map <spec.geas>=<記録>]...` は、統一形式の差分（`git diff` か `diff -u`）を読み、その差分が触る要件と、要件ごとの持ち主（誰が承認すべきか）、出どころ、最後に決めたことを答える。何も走らせず、何も書かない。差分を読むのは ritsu の土台の読み手（geas と同じもの。ritsu の DESIGN 4.14）で、コードについては geas が口（`Claims` の `affected`）で答える。geas は、記録が差分のどちらの側のものかを blob のハッシュで確かめ、合わない記録からは答えない（geas の DESIGN 7.5）。

差分が触るファイルを、ルートからのパスで一つずつ見る（改名なら前と後の二つのパス）。

1. **`.req`**：差分の行が入る要件のブロック（要件の行から、その下の字下げした行まで）を、変わった要件として挙げる。ディスクのファイルは、差分を当てる前でも当てたあとでもよく、どちらにも合わなければ exit 2 にする。要件の文か期間の行が変わったなら、その要件のリンクが確かめ直しになることも言う。どの要件にも入らない行（役割、出典、範囲）が変われば、そう言う。
2. **出典のコピー**（yuen のコピー、借りた出典のコピー、リンクが指す規則とカレンダーが固定している条のコピー、`file` の出典のファイル）：その条を引く要件（借りた出典を通るものも、同じ条を自分で保存したものも）と、その条を固定している成果物（3.3）。
3. **成果物のファイル**（リンクが指すもののファイル）：そのファイルの中のものを指すリンクの要件を、満たす側と確かめる側に分けて。
4. **コード**：リンクに出てくる geas の spec ごとに、geas に差分と記録を渡す。`--map <spec.geas>=<記録>` はその spec の記録で、二度書けば変更の前と後の二つ、書かなければ spec の隣の `.geas/` の記録である。spec と記録のパスは、ほかのフラグと同じく走らせた場所から書く。geas が答えた主張ごとに、その主張か spec のファイルを指すリンクの要件を挙げる。geas が「どの主張も走らせない」と答えた行（unclaimed）は、そのファイルをリンクが指していればその要件に届き、指していなければ、どの要件にも届かない変更である。
5. **範囲のファイル**：範囲（1.8）の中で、どの要件にも辿れないファイル（E404 のもの）の変更は、どの要件にも届かない変更である。
6. **そのほか**：要件に関わらないファイルとして並べる。

最後に、触った要件を一つずつ、場所と持ち主と出どころと最後に決めたこと（日付のいちばん新しい `decided`）と一緒に挙げ、持ち主を並べる。`from` の無い要件（人が決めただけの要件）は、出どころの部分を書かず、決めたことだけを書く（ritsu の F.1 で、例 `greeter` の要件が `from ;` と空の出どころを書いていたのを直した。出力が変わるのは `from` の無い要件のときだけで、替える前のテストの材料には無かった）。

テストの材料 `tests/fixtures/geas`（geas の例 greeter の要件。記録は `geas map` で一度だけ取ってテストの材料に置いた）に、A の段階と同じ差分（`server.py` の、空の名前を受け付けないときの文言を一行変えたもの）と、変更の前と後の記録を渡すと、次のように答える。

```
$ yuen affected tests/fixtures/geas --root tests/fixtures/geas --diff tests/fixtures/geas/changes/change.diff --map tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/greeter/.geas/greeter.map.jsonl --map tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/changes/after.map.jsonl
diff: tests/fixtures/geas/changes/change.diff
the claims the change touches (geas "greeter/greeter.geas"; records: tests/fixtures/geas/greeter/.geas/greeter.map.jsonl (before), tests/fixtures/geas/changes/after.map.jsonl (after)):
  claim "rejects an empty name": tests/fixtures/geas/greeter/server.py 26 (after), 26 (before)
    checked by rejects_an_empty_name
files that requirements name, that the diff touches:
  file "greeter/server.py": met by greets_by_name, rejects_an_empty_name, totals_accumulate, unknown_paths_are_404
changes no requirement reaches: none
requirements touched:
  greets_by_name (tests/fixtures/geas/greeter.req:11): owner api; from contract
  rejects_an_empty_name (tests/fixtures/geas/greeter.req:21): owner api; from contract; decided 2026-10-03 by api: "an empty name is a client error, not a greeting"
  totals_accumulate (tests/fixtures/geas/greeter.req:32): owner api; from contract
  unknown_paths_are_404 (tests/fixtures/geas/greeter.req:42): owner api; from contract
4 requirements touched; ask api
```

前の記録だけを渡すと、geas は差分が足した行を引ける記録が無いと言い（geas の E063）、yuen はそれを主張の節に書き、ほかの節も答えてから exit 2 で終わる。

借りた出典のコピーを変える差分（koyomi のカレンダーが固定している 142 条のコピーを一文字変えたもの）と、規則を変える差分（軽減の表の一行を変えたもの）には、次のように答える。英語の材料では、`tests/fixtures/calendar_sources` のカレンダーが固定している英国の祝日の表（`bank-holidays.json`）の一件の日付を変えた差分と、`tests/fixtures/fee_rules` の小さな事業体の表の一行を変えた差分が、同じ形で答えられる。

```
$ yuen affected tests/fixtures/calendar_sources --root tests/fixtures/calendar_sources --diff tests/fixtures/calendar_sources/changes/copy.diff
diff: tests/fixtures/calendar_sources/changes/copy.diff
copies of sources the diff touches:
  tests/fixtures/calendar_sources/calendars/data/bank-holidays.json (bank_holidays): cited by last_day_moved
changes no requirement reaches: none
requirements touched:
  last_day_moved (tests/fixtures/calendar_sources/period_of_months.req:31): owner legal; from bank_holidays; decided 2026-10-03 by legal: "The day after is taken as it is, and not moved on if it is closed too. Decided for this material"
1 requirement touched; ask legal
$ yuen affected tests/fixtures/fee_rules --root tests/fixtures/fee_rules --diff tests/fixtures/fee_rules/changes/rule.diff
diff: tests/fixtures/fee_rules/changes/rule.diff
files that requirements name, that the diff touches:
  rulec "rules/extension_fees.rule": met by standard_fee, small_entity_fee
changes no requirement reaches: none
requirements touched:
  standard_fee (tests/fixtures/fee_rules/extension_fees.req:13): owner legal; from cfr §1.17
  small_entity_fee (tests/fixtures/fee_rules/extension_fees.req:26): owner legal; from cfr §1.27
2 requirements touched; ask legal
```

日本語の材料では、次のとおりである。

```
$ yuen affected tests/fixtures/koyomi --root tests/fixtures/koyomi --diff tests/fixtures/koyomi/changes/copy.diff
diff: tests/fixtures/koyomi/changes/copy.diff
copies of sources the diff touches:
  tests/fixtures/koyomi/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml (民法 第142条): cited by 満了日_142条; pinned by koyomi "民法の期間.cal"
changes no requirement reaches: none
requirements touched:
  満了日_142条 (tests/fixtures/koyomi/民法の期間.req:33): owner 法務; from 民法 第142条; decided 2026-10-03 by 法務: "「その翌日」は文字どおり末日の翌日と読み、翌日も休みでもそれ以上は動かさない。この材料のために決めたもので、法令の読み方を示すものではない"
1 requirement touched; ask 法務
$ yuen affected tests/fixtures/rulec --root tests/fixtures/rulec --diff tests/fixtures/rulec/changes/rule.diff
diff: tests/fixtures/rulec/changes/rule.diff
files that requirements name, that the diff touches:
  rulec "rules/印紙税の本則と軽減.rule": met by 本則の税額, 軽減税率
changes no requirement reaches: none
requirements touched:
  本則の税額 (tests/fixtures/rulec/印紙税.req:14): owner 税務; from 法 別表第一
  軽減税率 (tests/fixtures/rulec/印紙税.req:27): owner 税務; from 措置法 第91条
2 requirements touched; ask 税務
```

規則を変える差分では、規則のファイルの中のものを指す要件を全部挙げる。どの表が変わり、どのリンクが確かめ直しになるかは、差分を当てたあとの `check` が、表ごとの端（3.2）で正確に言う。

`--format json` は、同じ答えを一つの JSON にする。キーは `diff`、`root`、`requirement_files`、`copies`、`openspec_changes`（20 章で足した。OpenSpec の変更の提案）、`specs`、`files`、`unreached`、`others`、`requirements`、`owners`、`exit` の順で、パスはルートからの相対である（`tests/golden/affected/greeter.json`）。

exit code は、答えられて、要件の届かない変更が無ければ 0、要件の届かない変更があれば 1、差分が読めない、`--map` の spec をどのリンクも指していない、geas が記録を受け付けなかった、のどれかなら 2 にする。geas の `affected` の exit code（届かないコードがあれば 1）と同じ向きである。

**理由**：エージェントが差分を出したとき、人が知りたいのは「誰に見てもらえばよいか」と「その要件はどこから来たか」である。geas は差分から主張までを答え、yuen は主張から要件と持ち主と出典までを答える。二つを合わせると、差分 → 主張 → 要件 → 持ち主と出典、がつながる。

**捨てたもの**：

- `affected` が `.rule` や `.cal` の差分の中身を読み、どの出力が変わったかを言うこと。それには `.rule` を読み解くか、差分の前のファイルで言語を走らせることになる。ファイルの単位で要件を挙げ、どのリンクが確かめ直しになるかは、差分を当てたあとの `check` が正確に言う。
- geas の記録を yuen が自分で読み、差分の行と突き合わせること。geas の `affected` は、記録が差分のどちらの側のものかを blob のハッシュで確かめ、古い記録からは答えない（geas の DESIGN 7.5）。同じことを yuen に書けば、二つが食い違う。差分を読むところも、geas と同じ土台の読み手にした。
- 取り込む前に決めていた、geas の `affected` を子プロセスで呼んで JSON を読むこと（`geas affected <spec> <差分> --json`）。ritsu の中では口で尋ねる（P4）。

## 9. trace

**決定**：`yuen trace` は、一つの要件か成果物か出典の条から、「なぜこうなっているか」を、出典までたどって見せる。

- **要件**から：要件の文、期間、持ち主、版と置き換え。出どころ（出典の条は、コピーから引いた本文と、何年何月何日時点のどの版か。元になった要件は、その要件の出どころまで、さかのぼって）。決めたことを日付の順に。そのあとに、満たす成果物と確かめる主張と見送りを、確かめた記録と今の印と一緒に。成果物のファイルが固定している条（3.3）も添える。
- **成果物**から：それを指すリンクの要件ごとに、上の要件のたどり方。成果物のファイルが固定している条と、要件の出典が同じ本文かどうか（E107 と同じ比べ方）。
- **出典の条**から：その条を引く要件と、その条を固定している成果物。

英語の材料 `tests/fixtures/period_of_months` の `next_business_day` からたどると、次のとおりである。

```
$ yuen trace tests/fixtures/period_of_months --root tests/fixtures/period_of_months --requirement next_business_day
next_business_day (tests/fixtures/period_of_months/period_of_months.req:37), owned by legal, in force from 2026-10-01
  "When the last day fixed for taking an action falls on a Saturday, Sunday or Federal holiday, the action may be taken on the next succeeding business day which is not one"
  comes from cfr §1.7 (eCFR 37 CFR 1 as of 2026-01-01; the copy tests/fixtures/period_of_months/sources/law/37-CFR-1@2026-01-01/1.7.xml)
    > § 1.7 Times for taking action; Expiration on Saturday, Sunday or Federal holiday.
    > (a) Whenever periods of time are specified in this part in days, calendar days are intended. When the day, or the last day fixed by statute or by or under this part for taking any action or paying any fee in the United States Patent and Trademark Office falls on Saturday, Sunday, or on a Federal holiday within the District of Columbia, the action may be taken, or the fee paid, on the next succeeding business day which is not a Saturday, Sunday, or a Federal holiday. See § 90.3 of this chapter for time for appeal or for commencing civil action.
    > (b) If the day that is twelve months after the filing date of a provisional application under 35 U.S.C. 111(b) and § 1.53(c) falls on Saturday, Sunday, or on a Federal holiday within the District of Columbia, the period of pendency shall be extended to the next succeeding secular or business day which is not a Saturday, Sunday, or a Federal holiday.
    > [65 FR 14871, Mar. 20, 2000, as amended at 78 FR 62395, Oct. 21, 2013]
    looked at by legal on 2026-10-03; as it was looked at
  decided:
    2026-10-03 legal: The next succeeding business day is read as the day after the closed day, and not moved further if that day is closed too. Decided for this material; it does not say how any rule is read
  met by file "period_of_months.cal" — looked at by development on 2026-10-03; as it was looked at
  not verified: "koyomi's claim moved_on_or_after_last_day checks it, but this material reads the .cal as a file, so it does not name the claim" — approved by legal on 2026-10-03; as it was approved
```

日本語の材料 `tests/fixtures/period` の `満了日_142条` からたどると、次のとおりである。

```
$ yuen trace tests/fixtures/period --root tests/fixtures/period --requirement 満了日_142条 --lang ja
満了日_142条（tests/fixtures/period/民法の期間.req:37）持ち主 法務、2026-10-01 から効く
  「期間の末日が日曜日、国民の祝日に関する法律に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する」
  出どころ: 民法 第142条（e-Gov 129AC0000000089、2026-10-01 時点、版 129AC0000000089_20260624_508AC0000000045、コピーは tests/fixtures/period/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml）
    > 第百四十二条　期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。
    2026-10-03 に 法務 が確かめた。そのあと変わっていない
  決めたこと:
    2026-10-03 法務: 「その翌日」は文字どおり末日の翌日と読み、翌日も休みでもそれ以上は動かさない。この材料のために決めたもので、法令の読み方を示すものではない
  満たすもの: file "民法の期間.cal" — 2026-10-03 に 開発 が確かめた。そのあと変わっていない
  確かめるものを置かない:「koyomi の条件「142条の満了日は満了日以後」が確かめているが、この材料は .cal をファイルとして読むので、その条件を指さない」— 2026-10-03 に 法務 が承認した。そのあと変わっていない
```

成果物が規則かカレンダーなら、そのリンクの下に、成果物のファイルが固定している条（3.3）を添え、要件が同じ条を引いていれば、要件のコピーと同じ本文かどうか（E107 と同じ比べ方）も言う。一つの要件の中で、同じファイルの条は一度だけ添える。`tests/fixtures/rulec` の `軽減税率` からたどったときの、満たすものの行と、その下の二行は次のとおりである（`yuen trace tests/fixtures/rulec --root tests/fixtures/rulec --requirement 軽減税率 --lang ja` の出力の一部）。

```
  満たすもの: rulec "rules/印紙税の本則と軽減.rule" table 軽減 — 2026-10-04 に 開発 が確かめた。そのあと変わっていない
    rulec "rules/印紙税の本則と軽減.rule" が固定している条: 法 別表第一（e-Gov 342AC0000000023、2026-04-01 時点、sha256:0ba69792e960021e）
    rulec "rules/印紙税の本則と軽減.rule" が固定している条: 措置法 第91条（e-Gov 332AC0000000026、2026-04-01 時点、sha256:85faf53f6f6e8196、要件のコピーと同じ本文）
```

出典の条からたどると、その条を引く要件のあとに、その条を固定している成果物（リンクが指す規則とカレンダーのファイル）を、固定とコピーのパスと一緒に挙げる。成果物からたどると、どの要件も指していないときだけ、ファイルが固定している条を成果物の下に挙げる（指す要件があれば、その要件の中で添える）。

`--format json` は、同じ中身を、たどった順の木で出す。リンクごとの `pins`（ファイルが固定している条と、要件のコピーと同じ本文かどうかの `same_text`）、成果物から始めたときの `pins`、出典の条から始めたときの `pinned_by` を持つ。

## 10. doc

**決定**：`yuen doc` は、コードが実現すべきものを理解し、確かめる人（事業を回す人、経理や法務、運用する人、コードを見る開発者）と、来歴を監査する人が読むページを、Markdown か一枚の HTML で出す。書くのは `.req` と、検査の結果と、コピーから言えることだけである。

ページは次の順に並ぶ。

1. **見出し**：プロジェクトのファイルと、そのハッシュ、yuen のバージョン。検査の結果（通ったなら `check` の一行、印や欠けがあれば、その診断の見出しの並び）。
2. **トレーサビリティの表**：要件と版ごとに一行。名前、期間、出どころ、持ち主、満たすもの、確かめるもの、状態（確かめたまま、印あり、見送り）。
3. **出典**：出典ごとに、法令の ID と時点と版（`revision.txt`）、条ごとの固定、それを引く要件、それを固定している成果物。借りた出典は、どのファイルから借りたかを言う。
4. **要件ごとの「なぜ」**：要件の文、期間、版と置き換え、出どころ（出典の条文は、固定したコピーから項ごとに引用する。koyomi のページと同じ `article_lines` の形）、決めたこと（日付、誰が、なぜ）、リンクと確かめた記録（誰が、いつ）、見送りと承認、印があれば何が変わったかと差分。
5. **範囲**：範囲の成果物と、それに辿る要件。辿れないものがあれば、それを挙げる。
6. **確かめた記録の一覧**：日付の順に、誰がどのリンクを確かめ、どの見送りを承認したか。ハッシュも載せる。監査する人のための節である。

1〜4 の段（構文、名前、出典、成果物）にエラーがあればページを作らず、診断を標準エラーに出して exit 1。コピーや成果物が読めないまま整ったページを作れば、読む人を誤らせる。5〜7 の段のエラー（循環、期間、印、欠け、範囲）は、ページを読む人に見せるべき事実なので、ページを作って exit 0 にする（koyomi の 7.4 と同じ考え）。標準出力はページだけで、`yuen doc … > x.md` に診断は混ざらない。印のあるリンクの下には、`check` が出すのと同じ診断（差分つき）を載せる。

ページは一度ブロックの並び（見出し、段落、表、条文の引用、診断）として組み、Markdown と HTML はそれを書き出すだけにした（`src/doc/`。koyomi と同じ考えで、二つの形の中身が食い違わない）。グラフは `yuen api` が出すもの（11 章）から読み、条文は固定したコピーから、`yuen trace` と同じ ritsu-base の `quote_lines` で引く（e-Gov の条は `article_lines` の形、別表と eCFR の節は本文の行）。一つの条を二つの要件が引けば、二つめからは「上に引いた」と書く（印紙税の別表第一のように長い条を、版ごとに繰り返さないため）。出典の節の「固定している成果物」は、条を固定しているのがファイル全体なので、ファイルの参照で一度ずつ書く。`--out <dir>` は、最初の `.req` のファイル名から `<名前>.md` か `<名前>.html` を書き、書いたパスを一行で言う。HTML の枠（頭と配色の変数、`prefers-color-scheme` と `data-theme` の切り替え）は ritsu-base の `docpage` のもので、表の枠と引用と診断の見た目は yuen のものである。

HTML は一枚で、script も外のファイル（フォント、画像、スタイルシート）も読まない。明るい配色と暗い配色を CSS の変数で持ち、`prefers-color-scheme` に従い、`<html data-theme="light|dark">` で決められる（koyomi と chobo と同じ）。幅の狭い画面では、表がそれぞれの枠の中で横に動き、ページは横にはみ出さない。

**捨てたもの**：

- 来歴のグラフの図（Mermaid と SVG）。図に描けることは、トレーサビリティの表の一行と、要件ごとの節に全部ある。chobo と dandori の図は流れを見せるためのもので、yuen のつながりは、出典 → 要件 → 成果物の三段に収まる。要る例が出てから考える（18 章）。
- ページに成果物の中身（規則の表、日付の計算）を載せること。それは rulec と koyomi のページの仕事で、yuen のページは、そのページへ人を連れて行く（成果物の名前とファイルを書く）ところまでにする。

## 11. api

**決定**：`yuen api <path>...` は、プロジェクトのグラフ全体を JSON で出す。sakai や、ほかのツールが、CLI の出力だけから yuen を読めるようにする（ritsu の中のほかの言語は、yuen の口（`Items`、`References`）で読む）。1 と 2 の段にエラーがあれば出さない（exit 1）。印や欠けがあっても出し、状態として載せる。

テストの材料 `tests/fixtures/koyomi`（借りた出典で書き、koyomi の日付と条件を一つずつ指したもの）の `yuen api tests/fixtures/koyomi --root tests/fixtures/koyomi` は次のとおりである（実際の出力から、並びの中の一つずつを残して `…` で略し、要件のほかは一つのものを一行に詰めた。全部は `tests/golden/api/koyomi.json` にあり、テストが照らし合わせる）。

```json
{
  "yuen": "0.1.0",
  "root": "tests/fixtures/koyomi",
  "files": [{"path": "民法の期間.req", "name": "民法の期間", "version": "1", "sha256": "429c3f8856633f48"}, …],
  "roles": [{"name": "経理", "description": "支払の条件を決める"}, …],
  "sources": [{"file": "民法の期間.req", "name": "民法", "kind": "law", "db": "egov", "id": "129AC0000000089", "asof": "2026-10-01", "revision": "129AC0000000089_20260624_508AC0000000045", "borrowed": {"text": "koyomi \"民法の期間.cal\" source 民法", "tool": "koyomi", "path": "民法の期間.cal", "items": [["source", "民法"]]}, "pins": [{"fragment": "第140条", "sha256": "e880059021fbb67d"}, {"fragment": "第141条", "sha256": "0575c131b9f08063"}, {"fragment": "第142条", "sha256": "fc8c35a0769d3b35"}, {"fragment": "第143条", "sha256": "6950bdfb988439b6"}]}, …],
  "requirements": [
    {
      "name": "満了日_142条",
      "alias": "last_day_142",
      "version": 1,
      "file": "民法の期間.req",
      "line": 33,
      "text": "期間の末日が日曜日、国民の祝日に関する法律に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する",
      "in_force": {"from": "2026-10-01", "to": null},
      "owner": "法務",
      "replaces": [],
      "sha256": "d4f2d2a67322df17",
      "from": [{"line": 37, "sources": [{"source": "民法", "fragment": "第142条", "sha256": "fc8c35a0769d3b35"}], "requirement": null, "reviewed": {"date": "2026-10-04", "by": "法務", "up": ["fc8c35a0769d3b35"], "down": "d4f2d2a67322df17"}, "status": "ok"}],
      "decided": [{"date": "2026-10-03", "by": "法務", "why": "「その翌日」は文字どおり末日の翌日と読み、翌日も休みでもそれ以上は動かさない。この材料のために決めたもので、法令の読み方を示すものではない"}],
      "links": [{"line": 40, "role": "satisfied", "artifact": {"text": "koyomi \"民法の期間.cal\" date 満了日_142条", "tool": "koyomi", "path": "民法の期間.cal", "items": [["date", "満了日_142条"]]}, "sha256": "bba4761410179e6e", "reviewed": {"date": "2026-10-04", "by": "開発", "up": ["d4f2d2a67322df17"], "down": "bba4761410179e6e"}, "status": "ok"}, {"line": 42, "role": "verified", "artifact": {"text": "koyomi \"民法の期間.cal\" claim 142条の満了日は満了日以後", "tool": "koyomi", "path": "民法の期間.cal", "items": [["claim", "142条の満了日は満了日以後"]]}, "sha256": "7c616f5dcc9503a8", "reviewed": {"date": "2026-10-04", "by": "開発", "up": ["d4f2d2a67322df17"], "down": "7c616f5dcc9503a8"}, "status": "ok"}],
      "waivers": []
    },
    …
  ],
  "artifacts": [{"text": "koyomi \"民法の期間.cal\" date 満了日_142条", "tool": "koyomi", "path": "民法の期間.cal", "items": [["date", "満了日_142条"]], "sha256": "bba4761410179e6e", "end": "item", "pins": [{"db": "egov", "id": "129AC0000000089", "asof": "2026-10-01", "fragment": "第140条", "sha256": "e880059021fbb67d"}, {"db": "egov", "id": "129AC0000000089", "asof": "2026-10-01", "fragment": "第141条", "sha256": "0575c131b9f08063"}, {"db": "egov", "id": "129AC0000000089", "asof": "2026-10-01", "fragment": "第142条", "sha256": "fc8c35a0769d3b35"}, {"db": "egov", "id": "129AC0000000089", "asof": "2026-10-01", "fragment": "第143条", "sha256": "6950bdfb988439b6"}]}, …],
  "scopes": [{"file": "民法の期間.req", "line": 9, "text": "koyomi \"民法の期間.cal\" date", "artifacts": 3, "untraced": []}],
  "check": {"ok": true, "summary": "tests/fixtures/koyomi: ok — 4 requirements, whose 11 links and 1 waiver are as they were looked at; every requirement is met and checked, or waived; the 3 dates in scope all trace to a requirement", "diagnostics": []}
}
```

- キーはこの順に出す。`status` は `ok`、`unreviewed`、`up_changed`、`down_changed`、`unapproved`、`bad_record`（記録の形が崩れている）、`unreadable` のどれかで、見送りの承認のあとで要件が変わったものも `up_changed` にする。
- `sources` の一つは、`law` なら `db`、`id`、`asof`、`revision`、`pins`（条ごとの固定）を、`file` なら `path`（ルートからの相対）、`url`、`sha256` を持ち、借りた出典は `borrowed` に借りた先の参照を持つ（自分で保存した出典は `null`）。借りた出典の `pins` は、借りた先の言語の口が渡す固定である。
- `artifacts` の一つは、参照のキー（2.6）に、端のハッシュ `sha256`、`end`（端の中身がファイル全体なら `file`、そのもの一つの定義の文なら `item`。3.2）、`pins`（成果物のファイルが固定している条。3.3。規則とカレンダーのほかは空）を足したもの。
- `scopes` の一つは `file`、`line`、`text`、`artifacts`（数）、`untraced`（参照の並び）を持つ。
- 段階 B の実物は `tests/golden/api/period.json`、ほかの言語のものを指すテストの材料のものは `tests/golden/api/` のほかの JSON にある。

## 12. ReqIF

**決定**：`yuen export reqif` は、プロジェクトを一つの ReqIF 1.2 の文書（UTF-8 の XML、名前空間 `http://www.omg.org/spec/ReqIF/20110401/reqif.xsd`、拡張子 `.reqif`）に書き出す。企業の要件管理ツールに、要件と出典と成果物と、そのつながりを渡すためである。1〜4 の段にエラーがあれば、診断を標準エラーに出し、何も書かずに exit 1 にする（端が決まらないものがあるため）。5〜7 の段のエラー（循環、期間、印、欠け）は、状態として書き出して exit 0 にする。

| yuen | ReqIF |
|---|---|
| プロジェクト | `REQ-IF`。`THE-HEADER` の `REQ-IF-TOOL-ID` と `SOURCE-TOOL-ID` は `yuen <バージョン>`、`REQ-IF-VERSION` は `1.0`、`TITLE` はファイルの見出しの名前（ファイルが二つ以上なら `, ` でつなぐ） |
| `.req` のファイル | `SPECIFICATION`（型 `yuen requirements`、`LONG-NAME` は見出しの名前）を一つ。属性は `yuen.file`（ルートからのパス）、`yuen.version`（見出しの版）、`yuen.description`。要件の版を、ファイルの順に `SPEC-HIERARCHY` で並べる |
| 要件の版 | `SPEC-OBJECT`（型 `yuen requirement`）。`ReqIF.ForeignID`（文字列「別名 v1」）、`ReqIF.Name`（名前。版が二つ以上なら「支払日 v2」）、`ReqIF.Text`（要件の文）、`yuen.inForce`、`yuen.owner`、`yuen.decided`（決めたことの行を一行ずつ段落に）、`yuen.waived`（見送りの行と承認の記録の行を一行ずつ段落に）、`yuen.sha256`（端のハッシュ）、`yuen.status` |
| 出典の条 | `SPEC-OBJECT`（型 `yuen source`）。`ReqIF.ForeignID` と `ReqIF.Name`（「民法 第142条」）、`ReqIF.Text`（コピーの本文を、`trace` と同じく項ごとに段落に）、`yuen.law`（「egov 129AC0000000089」）、`yuen.asof`、`yuen.revision`、`yuen.sha256`（固定）。`file` の出典は `yuen.file`、`yuen.url`、`yuen.sha256`。`SPECIFICATION`「sources」（型 `yuen list`）に並べる |
| 成果物 | `SPEC-OBJECT`（型 `yuen artifact`）。`ReqIF.ForeignID` と `ReqIF.Name`（2.4 の文字の形）、`yuen.sha256`、`yuen.end`（`file` か `item`）。`SPECIFICATION`「artifacts」に並べる |
| `from` | `SPEC-RELATION`（型 `yuen from`）。`SOURCE` が要件、`TARGET` が出典の条か元になった要件。一行で二つの条を引いたら、条ごとに一つ |
| `satisfied by`、`verified by` | `SPEC-RELATION`（型 `yuen satisfied by`、`yuen verified by`）。`SOURCE` が要件、`TARGET` が成果物 |
| `replaces` | `SPEC-RELATION`（型 `yuen replaces`。属性は持たない）。`SOURCE` が置き換える要件、`TARGET` が置き換えられる要件 |
| 確かめた記録と印 | つながりの属性 `yuen.reviewedOn`、`yuen.reviewedBy`、`yuen.up`（記録が持つリンク元のハッシュのうち、そのつながりの先のもの）、`yuen.down`、`yuen.status`（`api` の `status` の語） |

- 属性の名前は、ReqIF Implementor Forum の実装ガイド（v1.10、2024-01-26）の 2.4 の決まりにそろえる。要件の識別子は `ReqIF.ForeignID`（`AttributeDefinitionString`）、短い名前は `ReqIF.Name`（XHTML）、本文は `ReqIF.Text`（XHTML）。出典の条と成果物にも `ReqIF.Name` を付け、どの型のものも、ツールの一覧で名前が読めるようにした。yuen だけの属性には `yuen.` を付ける。
- 要件の `yuen.status` は、その版のリンクと見送りがどれも確かめたときのままなら `ok` である。そうでなければ、そうでないリンクと見送りの `api` の語を、`unreviewed`、`up_changed`、`down_changed`、`unapproved`、`bad_record`、`unreadable` の順に、同じ語は一度だけ、空白で区切って並べる。
- `yuen.decided` と `yuen.waived` の段落は、`.req` の行そのもの（`decided 2026-10-03 by 法務 "…"`、`not verified "…"`、`approved 2026-10-03 by 法務 sha256:…`）にする。文面の言語に依らず、`.req` が書いていることだけを渡すためである。
- 仕様をまたぐつながりは、ガイドの 2.11 のとおり `RELATION-GROUP` に入れる。要件のファイル → sources、→ artifacts、→ 別の要件のファイル、の組ごとに一つで、`LONG-NAME` は「支払日 -> artifacts」のように書く。同じファイルの中の要件どうしのつながりは、どの組にも入れない。`RELATION-GROUP-TYPE` には属性を置かない（ガイドの 2.12）。
- 文字列の型の `MAX-LENGTH` は必ず書く決まりなので（ガイドの 2.13）、書き出す値のどれよりも長い値（100,000）にし、超える値があれば書き出さずに exit 2 にする。XML 1.0 に書けない文字（タブ、改行、復帰のほかの制御文字）を含む値も、書き出さずに exit 2 にする。
- 日付（期間、決めた日、確かめた日）は、ReqIF の日付の型（`xsd:dateTime`）ではなく文字列（`YYYY-MM-DD`）にする。yuen の日付は時刻もタイムゾーンも持たず、日付の型に直すと、`.req` が言っていない時刻とタイムゾーンを足すことになるからである。
- XHTML は `<xhtml:div>` と `<xhtml:p>` だけを使い、見出しの要素（`h1`〜`h6`）は使わない（ガイドの 2.8）。段落が一つなら `<xhtml:div>` の中にそのまま、二つ以上なら `<xhtml:p>` を並べる。
- **識別子**：`IDENTIFIER` は `_` に、`yuen/1`、種類、そのものの名前を NUL でつないだバイト列の SHA-256 の先頭 32 桁を続けたもの。名前は、要件なら名前と版、出典の条ならデータベースと ID と時点と条（同じ条でも、時点が違えば別の本文なので、時点も入れる）、`file` の出典ならパス、成果物なら文字の形、つながりなら種類と両端の識別子（同じつながりを二度書いたときは、二つめに番号を足す）、仕様なら見出しの名前か「sources」「artifacts」である。何度書き出しても同じ値になり、受け取った側は前に取り込んだものを更新できる。`xsd:ID` の決まり（数字で始まらない）にも合う。
- **時刻**：ReqIF は、どの要素にも `LAST-CHANGE`（`xsd:dateTime`）を求める。そのものに関わる日のいちばん新しいものの `T00:00:00Z` を書く。要件の版なら、決めた日と、そのリンクと見送りの記録の日。つながりなら、確かめた日。仕様と組なら、中のもののいちばん新しい日。日を持たないものと、`THE-HEADER` の `CREATION-TIME` には、`--time` の値か、プロジェクトに書かれた決めた日と確かめた日（承認した日を含む）のいちばん新しいものを書く。`in force` の日付は使わない。効き始める日であって何かが変わった日ではなく、先の日付であることも多いからである。そうした日が一つも無ければ、`--time` を求めて exit 2 にする。走らせた時刻は使わない。同じ `.req` から、いつ書き出しても同じバイト列になるようにするためである。

`tests/fixtures/period` の `満了日` は、一行の `from` で二つの条を引いている。この行は条ごとのつながりになり、そのうち 141 条へのつながりは次のとおりである（`tests/golden/export/period.reqif` から。テストが書き出したものと照らし合わせる）。`yuen.up` は、記録が持つ二つのハッシュのうち 141 条のもの、`yuen.down` は `満了日` の端のハッシュである。

```xml
<SPEC-RELATION IDENTIFIER="_7810875129fd3f884d62b3acce632f7e" LAST-CHANGE="2026-10-03T00:00:00Z">
  <VALUES>
    <ATTRIBUTE-VALUE-STRING THE-VALUE="2026-10-03">
      <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>_309e5c9fc4d3093cabfe553f89398883</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
    </ATTRIBUTE-VALUE-STRING>
    <ATTRIBUTE-VALUE-STRING THE-VALUE="法務">
      <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>_de5a8b06f4ed15039cb9a8ce229e3882</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
    </ATTRIBUTE-VALUE-STRING>
    <ATTRIBUTE-VALUE-STRING THE-VALUE="0575c131b9f08063">
      <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>_0412d94d758f9f274f57ea3801ecc1a5</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
    </ATTRIBUTE-VALUE-STRING>
    <ATTRIBUTE-VALUE-STRING THE-VALUE="465b83ed8c251406">
      <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>_c4d6a9290a375591cd9afdab52a93f83</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
    </ATTRIBUTE-VALUE-STRING>
    <ATTRIBUTE-VALUE-STRING THE-VALUE="ok">
      <DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>_444acc8a6cf372af8a7a57ed2c063fe4</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION>
    </ATTRIBUTE-VALUE-STRING>
  </VALUES>
  <SOURCE><SPEC-OBJECT-REF>_b8700228e15f67ee73ae42631eca2022</SPEC-OBJECT-REF></SOURCE>
  <TARGET><SPEC-OBJECT-REF>_41bb84d09b5f8588640f53db293c758c</SPEC-OBJECT-REF></TARGET>
  <TYPE><SPEC-RELATION-TYPE-REF>_38ebadf3c7922dafd9bd709143b69410</SPEC-RELATION-TYPE-REF></TYPE>
</SPEC-RELATION>
```

**確かめ方**：A の段階で、手で書いた小さな ReqIF（要件二つ、つながり一つ、仕様一つ）を二つのツールで確かめ、C の段階で、yuen が書き出したものを同じツールで確かめた（19 章）。

- xmllint（libxml 2.9.13）に OMG の `reqif.xsd` を渡して検証する。`reqif.xsd` は `driver.xsd` と W3C の `xml.xsd` を読み込み、`driver.xsd` は W3C の XHTML のスキーマを読み込むので、全部で 24 個のファイルになる。それを手元に取り、XML のカタログで URL を手元のファイルに向けて、`--nonet` で通信せずに検証できる。`LAST-CHANGE` を消した文書は落ちる。ところが、つながりの `TARGET`（`GLOBAL-REF` で、ただの文字列）を無い要件に向けた文書も、型の参照（`LOCAL-REF` で、`xsd:IDREF`）を無い識別子に向けた文書も、検証を通る。
- Python の `reqif` 0.1.0（StrictDoc のプロジェクトの ReqIF の読み手）の `reqif validate` は、無い要件を指すつながりを「non-existing」と言って exit 1 で落とす。型の参照の宛先が無いことと、`LAST-CHANGE` の欠けは言わない。`--use-reqif-schema` を付けると、`reqif` に入っているスキーマでも検証し、`LAST-CHANGE` の欠けを言って exit 1 で落とす。

だから C の段階のテストは、xmllint でスキーマを、`reqif validate` で参照とスキーマを確かめ、さらに yuen のテストが自分で、すべての `-REF` が、その名前の要素の識別子を指していること（`<SPEC-OBJECT-TYPE-REF>` なら `SPEC-OBJECT-TYPE` の識別子）を確かめる。24 個のスキーマのファイルは、OMG と W3C のもので、リポジトリには入れない。`tools/reqif/fetch.sh` が取ってきて SHA-256 を確かめ、カタログを書く。置き場所は引数で替えられ、テストは `YUEN_REQIF_XSD`、無ければ `tools/reqif/xsd` を見る。無ければ SKIP する。

**捨てたもの**：

- ガイドの 2.10 の「外の要素へのリンク」（`REQ-IF-TOOL-EXTENSION` に URI を書く拡張）で成果物を表すこと。拡張を読むツールは限られる。成果物を `SPEC-OBJECT` にすれば、どのツールも、ふつうの要素として取り込める。
- `from` の一行を、条がいくつあっても一つのつながりにすること。ReqIF のつながりは、元と先を一つずつしか持たない。
- ReqIF を読んで `.req` にすること。書き出しだけで始める（18 章）。
- 成果物のファイルが固定している条（3.3）を、ReqIF のつながりにすること。ReqIF に書くのは `.req` が書いていることとリンクで、規則やカレンダーが固定している条は、その言語が持つ事実である。書けば、成果物から出典へのつながりの組（`RELATION-GROUP`）がもう一つ要る。PROV には書く（13 章）。読む人が求めたら考える（18 章）。

## 13. W3C PROV

**決定**：`yuen export prov` は、プロジェクトの来歴を W3C PROV で書き出す。既定は PROV-N（W3C の勧告で、人が読める）、`--format json` で PROV-JSON（ツールが読む形。W3C のメンバー提出）。二つは同じ記録の並びから書くので、同じ文書になる。書き出さないとき（1〜4 の段のエラー）と、印や欠けの扱いは、ReqIF と同じである。

| yuen | PROV |
|---|---|
| 要件の版 | entity（`prov:type` は `yuen:Requirement`。`prov:label` に名前、`yuen:version`、`yuen:text`、`yuen:inForce`、`yuen:sha256`） |
| 出典の条 | entity（`yuen:Source`。`yuen:law`、`yuen:asof`、`yuen:revision`、`yuen:sha256`。`file` の出典は `yuen:file`、`yuen:url`、`yuen:sha256`） |
| 成果物 | entity（`yuen:Artifact`。`prov:label` に 2.4 の文字の形、`yuen:sha256`、`yuen:end`） |
| 役割 | agent（`yuen:Role`。`yuen:description`） |
| 決めたこと | activity（`yuen:Decision`。始まりは決めた日、`yuen:why`）。`wasAssociatedWith(決めたこと, 役割, -)`、`wasInfluencedBy(要件, 決めたこと)`、要件が出典を引いていれば、その条ごとに `used(決めたこと, 出典の条, -)` |
| `from @出典` | 条ごとに `wasDerivedFrom(要件, 出典の条, -, -, -, [prov:type='prov:PrimarySource'])` |
| `from <要件>` | `wasDerivedFrom(要件, 元の要件)` |
| 版と置き換え | `wasDerivedFrom(新しい版, 一つ前の版, -, -, -, [prov:type='prov:Revision'])`、`wasDerivedFrom(置き換える要件, 置き換えられる要件, -, -, -, [prov:type='prov:Revision'])` |
| 持ち主 | `wasAttributedTo(要件, 役割, [prov:type='yuen:owner'])` |
| `satisfied by`、`verified by` | `wasInfluencedBy(成果物, 要件, [prov:type='yuen:satisfies'])`（`yuen:verifies`） |
| 確かめた記録 | activity（`yuen:Review`。始まりは確かめた日、`yuen:link`（`from`、`satisfied by`、`verified by`）、`yuen:up`、`yuen:down`、`yuen:status`）。リンク元ごとに `used(確かめたこと, リンク元, -)`、`used(確かめたこと, リンク先, -)`、`wasAssociatedWith(確かめたこと, 役割, -)`。記録の無いリンクには書かない |
| 見送り | activity（`yuen:Waiver`。`yuen:side`、`yuen:why`、`yuen:status`。承認していれば、始まりは承認した日で、`yuen:sha256` に承認の記録の要件のハッシュ）。`used(見送り, 要件, -)`、承認していれば `wasAssociatedWith(見送り, 役割, -)` |
| 成果物が固定している条（3.3） | `wasInfluencedBy(成果物のファイル, 出典の条, [prov:type='yuen:pins'])`。固定はファイルの単位なので、ファイルの entity から引く。リンクがファイルそのものを指していなければ、そのファイルを entity（`yuen:Artifact`、`yuen:end="file"`、`yuen:sha256` はファイルのバイト列のハッシュ）として足し、`.req` が宣言していない条は、出典の条の entity として足す（ritsu の D.7） |

- 決めたことを、要件の生成（`wasGeneratedBy`）ではなく影響（`wasInfluencedBy`）で書くのは、決めたことが、要件の版を一度だけ作る行いではなく、あとから何度でも重ねる記録だからである。
- `hadPrimarySource` は PROV-DM の関係の名前だが、PROV-JSON には同じ名前の項が無い（Python の `prov` は `'hadPrimarySource' is not a recognised PROV-N record-type keyword` と言って読まなかった。19 章）。`wasDerivedFrom` に `prov:type='prov:PrimarySource'` を付けて書く。
- 名前空間は二つ置く。型と属性の `yuen`（`https://i2y.github.io/ritsu/ns/yuen#`）と、ものの識別子の `y`（`urn:yuen:`）。要件は別名で `y:requirement/<別名>/v<n>`、役割は名前で `y:role/<名前>`（名前に、PROV-N の修飾名に書けない文字があればハッシュ）とする。出典の条と成果物は、名前に PROV-N で使えない文字（空白、`"`）を含むので、ReqIF と同じ作り方のハッシュで `y:source/<32 桁>`、`y:artifact/<32 桁>` とする。決めたこと、確かめたこと、見送りは、その中身（要件、日付、役割、理由、記録のハッシュ）のハッシュで `y:decision/…`、`y:review/…`、`y:waiver/…` とする。確かめ直したり承認し直したりすれば、別の行いとして別の識別子になる。entity と agent には `prov:label` に名前を書く。
- `yuen` の IRI は、リポジトリの URL ではなく、ritsu のサイトの URL にした。理由は二つある。配るのは ritsu だけで、yuen のリポジトリは作らないこと。語の IRI を開いたときに、その語の説明が読めるようにしたいこと。配る前に決めたので、替えて困る文書は無い。ページは `website/docs/ns/yuen.md`（日本語は `website/docs-ja/ns/yuen.md`）で、公開先は `https://i2y.github.io/ritsu/ns/yuen/` である。語の IRI は名前空間に語を続けたもの（`https://i2y.github.io/ritsu/ns/yuen#Requirement`）で、`#` の前に `/` が無い。そのためサーバーは `…/ns/yuen` を `…/ns/yuen/` へ 301 で送り（GitHub Pages がディレクトリの URL にそうすることは、rulec のサイトで確かめた）、ブラウザは `#Requirement` を持ち越す。id の照合は大文字小文字を区別するので、Zensical の既定の id（小文字）では `#inForce` が着かない。そこで見出しは `### inForce { #inForce }` のように書き、id を語の綴りそのままにしてある。ローカルに建てたサイトを Chrome で開いて、`#Requirement` と `#inForce` がその語の項に着き、`#inforce` は着かないことを確かめた。語の一覧は `src/export/prov.rs` の `TERMS`（型、関係の種類、属性と、書かれる先）で、yuen の `tests/export.rs` が、全プロジェクトの書き出しにある語と一覧が同じことを、ritsu の `tests/website.rs` が、一覧とページ（英語と日本語）の語が同じことと、ページの例がコマンドの出力であることを確かめる。
- 関係の記録には識別子を付けない。PROV-JSON では `_:r1` のような空白ノードの名前で並べ、`prov` はそれを識別子の無い記録として読む（PROV-N の識別子の無い記録と等しくなる）。
- 時刻は、yuen の日付に `T00:00:00` を付けたもの（タイムゾーンを書かない）。

テストの材料 `tests/fixtures/ecfr`（英語の要件一つと、その二つの見送り）は、次のように書き出される。

```
$ yuen export prov tests/fixtures/ecfr --root tests/fixtures/ecfr
document
  prefix yuen <https://i2y.github.io/ritsu/ns/yuen#>
  prefix y <urn:yuen:>

  agent(y:role/safety, [prov:type='yuen:Role', prov:label="safety", yuen:description="decides how the regulation reads"])
  entity(y:source/7e4bbf45001c614fa29502d3bc2fa12e, [prov:type='yuen:Source', prov:label="osha §1910.157", yuen:law="ecfr 29 CFR 1910", yuen:asof="2026-01-01", yuen:sha256="c2a9ce966c7e2269"])
  entity(y:requirement/extinguisher_distance/v1, [prov:type='yuen:Requirement', prov:label="extinguisher_distance", yuen:version="1", yuen:text="No employee travels more than 75 feet to a portable fire extinguisher for Class A fires", yuen:sha256="08a4819829372b9b"])
  wasAttributedTo(y:requirement/extinguisher_distance/v1, y:role/safety, [prov:type='yuen:owner'])
  wasDerivedFrom(y:requirement/extinguisher_distance/v1, y:source/7e4bbf45001c614fa29502d3bc2fa12e, -, -, -, [prov:type='prov:PrimarySource'])
  activity(y:review/e8256013bfdb91ba1b18e75857e38792, 2026-10-03T00:00:00, -, [prov:type='yuen:Review', yuen:link="from", yuen:up="c2a9ce966c7e2269", yuen:down="08a4819829372b9b", yuen:status="ok"])
  used(y:review/e8256013bfdb91ba1b18e75857e38792, y:source/7e4bbf45001c614fa29502d3bc2fa12e, -)
  used(y:review/e8256013bfdb91ba1b18e75857e38792, y:requirement/extinguisher_distance/v1, -)
  wasAssociatedWith(y:review/e8256013bfdb91ba1b18e75857e38792, y:role/safety, -)
  activity(y:waiver/4e902a59567f475e5b5593df6d352e3e, 2026-10-03T00:00:00, -, [prov:type='yuen:Waiver', yuen:side="not satisfied", yuen:why="The test material names no artifact", yuen:sha256="08a4819829372b9b", yuen:status="ok"])
  used(y:waiver/4e902a59567f475e5b5593df6d352e3e, y:requirement/extinguisher_distance/v1, -)
  wasAssociatedWith(y:waiver/4e902a59567f475e5b5593df6d352e3e, y:role/safety, -)
  activity(y:waiver/59d5526e231fd907efa661ba6466811e, 2026-10-03T00:00:00, -, [prov:type='yuen:Waiver', yuen:side="not verified", yuen:why="The test material names no claim", yuen:sha256="08a4819829372b9b", yuen:status="ok"])
  used(y:waiver/59d5526e231fd907efa661ba6466811e, y:requirement/extinguisher_distance/v1, -)
  wasAssociatedWith(y:waiver/59d5526e231fd907efa661ba6466811e, y:role/safety, -)
endDocument
```

**確かめ方**：A の段階で、要件一つ、出典の条一つ、決めたこと一つ、役割一つの小さな文書を PROV-JSON で手で書き、Python の `prov` 3.2.2（Python 3.13.11 の venv）で読んだ（19 章）。C の段階のテストは、テストの材料の三つのプロジェクト（`period`、`ecfr`、`payment`）を PROV-N と PROV-JSON で書き出し、`prov` で両方を読んで（PROV-N は、勧告の文法だけを受け付ける `strict` で読む）、二つが等しいことと、種類ごとの記録の数が `yuen api` から数えた数と合うことを確かめる。三つとも通った（記録は 58、15、79 件）。ritsu の D.7 で、ほかの言語のものを指すテストの材料七つ（`rulec`、`koyomi`、`chobo`、`geas`、`proto`、`dandori`、`sakai`）を足し、固定の辺（`yuen:pins`）も `yuen api` の `pins` から数えるようにした。七つとも通った（記録は 72、92、57、80、63、57、34 件）。ReqIF も、十のプロジェクトのどれも、xmllint と `reqif validate`（`--use-reqif-schema` も）を通った。`prov` が無ければ SKIP する。

**捨てたもの**：PROV-O（Turtle）。PROV-N と PROV-JSON で、人とツールの両方に届く。RDF の読み手を持つ人は、`prov` などで変換できる。

## 14. 出典のコマンド

**決定**：rulec と koyomi と同じく三つ。通信するのは `fetch` と `outdated` だけで、`curl` を子プロセスで呼ぶ。HTTP と HTTPS は三度まで試し（二度めの前に 2 秒、三度めの前に 4 秒待つ）、`file://` は一度だけ読む（koyomi の 9 章と同じ）。`curl` そのものが見つからなければ、試し直さずに exit 2 にする。どのコマンドも、`.req` を構文と名前の段まで読み、検査は通さない。コピーが無い・固定が無い・固定と違う、は、どれもこのコマンドで直すものだからである。構文か名前にエラーがあれば、診断を標準エラーに出して exit 1 にする。

- `yuen source fetch <path>...`：自分の `law` の出典の、固定している条と引いている条を、一つずつ取ってきて保存する。e-Gov は `law_data/<ID>?asof=<日付>&elm=<要素>&law_full_text_format=xml`（`law_full_text` の base64 を戻したものがコピーで、版の ID を `revision.txt` に書く）。要素はコピーのファイルの名前から作る（`MainProvision-Article_142`。別表は `AppdxTable[1]`、改正法の附則は、その法令の附則を文書の順に数えた位置 `SupplProvision[k]` で、位置は法令全体を一度読んで決める。rulec と同じ）。eCFR は `versioner/v1/full/<日付>/title-<title>.xml?part=<part>&section=<section>`（rulec と同じ場所）。本文（タグを落としたもの）が前のコピーと同じなら、コピーを書き換えない（rulec の §15.71、koyomi の 9 章）。`file` の出典は `url` から取って `file` に書く。借りた出典は取らず、「koyomi source fetch で取る」のように、それを宣言したツールのコマンドを言う。
- `yuen source pin <path>...`：コピーの SHA-256 の先頭 16 桁を、固定の行に書く。変えるのはその 16 桁だけで、ほかの文字は、行の終わりの CR LF も含めて一字も変えない。引いているのに固定の行が無い条には、その出典の最後の固定の行のあとに一行を足す。コピーが無い条は固定できないと言い、`source fetch` を先に走らせるよう言う。借りた出典は固定しない。
- `yuen source outdated <path>...`：e-Gov は `law_revisions/<ID>` で、`asof` より後に施行される版を引き、その施行日の本文を一つ前の本文と比べる（同じ日に施行される版は一つにまとめ、改正は施行の日に一度だけ言う。属性だけの書き換えは改正としない）。eCFR は `versioner/v1/versions/title-<title>.json?part=<part>&section=<section>` で、`asof` より後の `amendment_date` を持ち、`substantive` が真の版だけを引く（rulec と同じ）。`file` の出典は、`url` から取ったもののハッシュを固定と比べ、違えばコピーとの差分を見せる（どちらも 1 MiB までの UTF-8 のとき）。変わるものがあれば、施行日と版と本文の差分に加えて、**それを引く要件と持ち主、その要件を元にした要件、確かめ直しになるリンクと見送りの数、その条を固定している成果物**（3.3。リンクが指す規則とカレンダー）を言い、exit 1。借りた出典は、借りた先の言語の口（`Sources`）が渡す固定と、そのファイルの隣のコピーで、自分の出典と同じに問う。

確かめ直しになるものの数は、印の付き方（4.3）から決まる。条が変われば、それを引く要件の端が変わり（4.1）、その要件を元にした要件の端も、たどれる限り変わる。端の変わった要件は、その `from` の行（変わった条を引く行はリンク元が、ほかの行はリンク先が変わる）、`satisfied by` と `verified by` のリンク、見送りの全部に印が付く。その数を数える。

借りた出典の改正も、ritsu の D.7 から問う（取り込む前は、借りた出典の固定をツールの JSON から読むつもりで、それまでは「まだ借りた出典を読めない」と言って exit 2 にしていた）。取り直して固定し直すのは借りた先の言語なので、最後の行は、そのファイルの `source fetch` と `source pin` を言う。yuen のクレートのバイナリは、借りた先の言語を読めないので、`ritsu yuen source outdated …` で走らせるよう言って exit 2 にする。

借りた出典の 142 条が、ある版で一文字変わると、次のように言う（`crates/ritsu/tests/yuen.rs` の中の HTTP サーバーが、その版の 142 条だけ「その翌々日」に書き換えて返したときの、`ritsu yuen` の出力）。

```
民法: the revision in force from 2027-06-23 (129AC0000000089_20270623_TEST) leaves the cited articles as they are
民法: the revision in force from 2028-06-13 (129AC0000000089_20280613_TEST) changes 第142条
  what changed in the text of 第142条:
      @@ -1,2 +1,2 @@
        第百四十二条
      - 期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。
      + 期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌々日に満了する。
  cited by: 満了日_142条 (owned by 法務, koyomi/民法の期間.req:33)
  to look at again: 3 links
  pinned by: koyomi "民法の期間.cal"
  in koyomi "民法の期間.cal" source 民法, move asof to 2028-06-13, then run koyomi source fetch and koyomi source pin; yuen check then marks these
```

2026-10-03 に、本物の e-Gov に `outdated` を一度問い合わせた。民法の 140〜143 条は、2026-10-01 より後に施行される五つの版のどれでも変わらない（koyomi の DESIGN 9 章が 2026-10-02 に見たことと同じ）。テストは通信しないので、この塊は走らせない。

```
$ yuen source outdated tests/fixtures/period --root tests/fixtures/period --lang ja
民法: 2027-06-23 施行の版（129AC0000000089_20270623_508AC0000000045）では、引いている条は変わりません
民法: 2027-12-05 施行の版（129AC0000000089_20271205_507AC0000000057）では、引いている条は変わりません
民法: 2028-06-13 施行の版（129AC0000000089_20280613_505AC0000000053）では、引いている条は変わりません
民法: 2028-12-23 施行の版（129AC0000000089_20281223_508AC0000000045）では、引いている条は変わりません
民法: 2029-06-23 施行の版（129AC0000000089_20290623_508AC0000000045）では、引いている条は変わりません
```

ある版で 142 条の本文が一文字変わると、次のように言う（`tests/fetch.rs` の中の HTTP サーバーが、その版の 142 条だけ「その翌々日」に書き換えて返したときの出力）。

```
民法: 2027-06-23 施行の版（129AC0000000089_20270623_TEST）では、引いている条は変わりません
民法: 2027-12-05 施行の版（129AC0000000089_20271205_TEST）では、引いている条は変わりません
民法: 2028-06-13 施行の版（129AC0000000089_20280613_TEST）で 第142条 が変わります
  第142条 の本文の差分:
      @@ -1,2 +1,2 @@
        第百四十二条
      - 期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌日に満了する。
      + 期間の末日が日曜日、国民の祝日に関する法律（昭和二十三年法律第百七十八号）に規定する休日その他の休日に当たるときは、その日に取引をしない慣習がある場合に限り、期間は、その翌々日に満了する。
  引いている要件: 満了日_142条（持ち主 法務、period/民法の期間.req:37）
  確かめ直すもの: リンク 2 本と見送り 1 件
  asof を 2028-06-13 に進めて yuen source fetch と yuen source pin を走らせると、yuen check がこれらに印を付けます
民法: 2028-12-23 施行の版（129AC0000000089_20281223_TEST）では、引いている条は変わりません
民法: 2029-06-23 施行の版（129AC0000000089_20290623_TEST）では、引いている条は変わりません
```

exit code は rulec と koyomi と同じにする。`fetch` と `pin` は済めば 0（コピーが無くて固定できない条があっても、そう言って 0）、`outdated` は元が変わっていれば 1、`.req` が構文か名前の段で読めなければ 1、引数の誤りと `curl` の失敗と、つながっていない言語から借りた出典（`outdated`）は 2。

`check` は通信しないので、改正を知る手段は `outdated` だけである。CI の定期実行に置く。

テストは通信しない。e-Gov と eCFR の場所は `YUEN_EGOV` と `YUEN_ECFR` で替えられるようにし、テストの中に小さな HTTP サーバー（`127.0.0.1` の空いたポート。テストの終わりに止める）を立てて、用意したレスポンスを返す。`file` の出典の `url` は `file://` にする。本物の e-Gov と eCFR に問い合わせるテストは `YUEN_NET=1` のときだけ走らせ、走らせないときは `SKIP:` ではなく `not asked:` の行を出す（koyomi と同じ。ツールが欠けているのではなく、通信しないことを既定にしているからである）。DESIGN の中の `$ yuen source fetch` と `$ yuen source outdated` の塊も、同じ理由で `tests/design.rs` は走らせない。

## 15. 例

`examples/` に、一式の例をコピーして使う（元のファイルは ritsu の各言語のクレートにあり、コピーしたファイルの元の場所は、それぞれの例の README に書く）。どれも、公開されている出典（e-Gov の民法・印紙税法・租税特別措置法、eCFR の 29 CFR 1910、内閣府の祝日の表）か、例として決めた決まりだけを使う。法令を引く例は、どれも「例として書いたもので、法令の読み方を示すものではない」と `description` に書く。例として決めた決まりは、`decided` の理由に「この例のために決めたもの」と書く。

ritsu の決まりで、例は英語のものを先に置く。英語で作れるものは英語の `.req` を作り、日本語の版を `<名前>.ja.req` として横に置く（dandori の `<名前>.ja.flow` と同じ形）。e-Gov の法令にしか無い例（民法の期間、その読み方を変えた版、印紙税）は日本語だけで、ディレクトリの名前は英語、ファイルは `<英語の名前>.ja.req` である。一つの `.req` が一つのプロジェクトで、英語の版と日本語の版は同じディレクトリにあっても別のプロジェクトである（`check` には `.req` を一つずつ渡す。二つをまとめて渡すと、要件の別名がぶつかる）。

| 例 | コピーするもの（元） | 書くこと |
|---|---|---|
| `osha/` | rulec の `tests/corpus/osha_extinguisher.rule` と、その出典のコピー（`sources/law/29-CFR-1910@2026-01-01/1910.157.xml`） | 英語の、法令を引く例。出典は eCFR（`law ecfr "29 CFR 1910"`）で、yuen が自分で保存して固定する（コピーは規則のコピーと同じバイト列で、E107 は出ない）。満たすのは規則の表 `distance`、確かめるのは `rulec "osha_extinguisher.rule"`（規則の検査）。範囲は規則の表 |
| `greeter/` | geas の `examples/greeter/`（`greeter.geas`、`greeter.ja.geas`、`server.py`）と、`geas map <spec> --root .` が書いた記録 | 英語の `greeter.req` と日本語の版 `greeter.ja.req`。要件は主張ごとに一つで、出どころは `decided`。満たすのは `file "server.py"`、確かめるのは geas の主張。範囲の `server.py` に、geas の記録を通って辿る。`affected` の例の差分（`changes/change.diff`）と、変更のあとの記録（`changes/after.map.jsonl`） |
| `payment_terms/` | koyomi の `payment_20th_close_next_10th.cal`（と `.ja.cal`）、`calendars/tokyo_business_days.cal`（と `東京の営業日.cal`）、`calendars/data/syukujitsu.csv` | 支払日の要件（例として決めた決まり）と、営業日の要件（出典はカレンダーが固定する内閣府の祝日の表を借りる）。確かめるのは koyomi の条件（`within_60_days_of_receipt`、`paid_on_a_business_day`）。範囲は `.cal` の日付 |
| `refunds/` | chobo の `examples/refunds/refunds.book` と `refunds.ja.book` | 「返金は売上を超えない」の要件（例として決めた決まり）を、勘定一つと振替二つが満たし、帳簿の検査が確かめる。端の中身がそのもの一つの定義になる例。範囲は帳簿の振替 |
| `refund_contracts/` | テストの材料 `tests/fixtures/refund_contracts` と `contracts` の文書とポリシー（どれもこの例のために書いたもの） | 店の返金の要件（例として決めた決まり）を、OpenAPI の文書の操作とプロパティ、AsyncAPI の文書の操作、Cedar のポリシーとアクションが満たす（3.6）。英語の `refund_contracts.req` と日本語の版 `refund_contracts.ja.req` が同じディレクトリにある。確かめる側は、どれも見送り |
| `civil_code_periods/` | koyomi の `civil_code_period_end.ja.cal`、`calendars/民法142条の休日.cal`、`calendars/data/syukujitsu.csv`、`sources/law/129AC0000000089@2026-10-01/` | 1.1 の三つの要件（`civil_code_periods.ja.req`）。出典は koyomi から借りる。142 条の「その翌日」の読み方を、決めたこととして記録する（下） |
| `civil_code_periods_reread/` | 同じもの。`.cal` だけ、`満了日_142条` を `roll following`（休みが明けるまで動かす読み方）に書き換えたもの | わざと止まる例。`.req`（`civil_code_periods_reread.ja.req`）と `reviewed/` は `civil_code_periods/` と同じバイト列で、`check` が E303 で日付の定義の差分を見せる |
| `stamp_tax/` | rulec の `tests/corpus/印紙税の本則と軽減.rule` と、その出典のコピー（`sources/law/342AC0000000023@2026-04-01/`、`332AC0000000026@2026-04-01/`） | 契約書の印紙税額の要件を二つの版で書く（v1 は軽減の期間 `2014-04-01..2027-03-31` で、別表第一と措置法 91 条から。v2 は `2027-04-01..` で、別表第一から）。期間の始まりは規則の入力 `作成日` の範囲の始まり、軽減の終わりは規則の `define 軽減期間` にそろえた。満たすのは `output 印紙税額`、確かめるのは規則の検査。出典は yuen が自分で保存して固定する（コピーは規則のコピーと同じ本文で、E107 は出ない） |

**142 条の読み方**：koyomi の例は、民法 142 条の「その翌日」を、文字どおり翌日とする読み方（`if closed + 1 day`）と、休みが明けるまで動かす読み方（`roll following`）の二つを並べ、どちらを採るかは人が決める、として残した（koyomi の DESIGN 1.8 と 10 章）。`civil_code_periods.ja.req` は、その決めごとを `decided 2026-10-03 by 法務 "…"` として記録する（1.1）。決めたのは例の中の役割で、法令の読み方を示すものではない、と理由の文と `description` に書く。

`civil_code_periods_reread/` は、決めたあとで `.cal` が別の読み方に書き換えられたときに、何が起きるかを見せる。`check` は、`満了日_142条` を満たすリンクに E303 を一つだけ出し、確かめたときの定義の文との差分（`if closed + 1 day` が `roll following` に変わったこと）を見せる。ritsu の D.7 から、koyomi の日付と条件の端はファイル全体ではなく、そのものの定義の文なので（3.2）、同じ `.cal` の `起算日` と `満了日` と、条件 `142条の満了日は満了日以後`（定義の文は `満了日_142条 >= 満了日` のまま）へのリンクには印が付かない。A の段階では、日付の端をファイル全体にするつもりで、三本に E303 が付き、差分は一本めだけに出ると書いていた。書き換えたものだけが止まり、読み直すべきものが一つに絞られるのは、端をそのもの一つにした効き目である。

わざと止まる例は `civil_code_periods_reread/` 一つで、README とテスト（`tests/examples.rs`）は、ほかの例が通り、これだけが E303 一つで止まることを確かめる。どの例にも、`review --all` で書く記録が残っていないことも確かめる（止まる例を除く）。

## 16. 実装

- Rust（edition 2024、手元の stable 1.94.1 で通ること）。依存は ritsu の土台のクレート（ritsu-base、ritsu-ports、ritsu-proto）と serde_json だけ（`preserve_order` の機能を使い、`api` と `--format json` のキーをこの文書の順に出す）。ほかの言語のクレートには依存しない（ritsu の DESIGN 3.1）。
- SHA-256 と base64（e-Gov の `law_full_text`）は ritsu-base のもの（依存を足さずに書いたもので、SHA-256 は FIPS 180-4 の既知の値でテストしている）を使う（16.1）。
- 法令のコピーの XML から本文を取り出す読み手は ritsu-base のもの（rulec の `xml_text` と koyomi の `article_lines` を一つにしたもの）を使う。`.proto` は ritsu の読み手（ritsu-proto）で読む（3.4）。ReqIF と PROV は、エスケープを自前でして書く。
- 見せる差分は、行の LCS で自前で作る（統一形式、前後二行、40 行を超えれば「ほか N 行」）。`affected` が読む統一形式の差分は、ritsu-base の読み手（geas と同じもの）で読む。
- ほかの言語は、ritsu の口で、同じプロセスの中で読む。口は yuen を走らせる側が渡す（`src/suite.rs` の `Suite`。中のものはプロジェクトの索引 `Index` で引く。`ritsu yuen` は ritsu-project が一度つないだすべての言語を渡し、yuen のクレートのバイナリは何も渡さない）。コマンドは関数（`yuen::run::run(引数, Suite, 標準出力, 標準エラー)`）で、クレートのバイナリはそれを呼ぶだけである。通信は `curl` を子プロセスで（`source fetch` と `source outdated` だけ）。
- 診断の文面は `tr!` で英語と日本語を隣に書く。台帳は `src/codes.rs`。
- テストの共通の部分（一時ディレクトリ、golden、SKIP、ツールの探し方、テストの中の HTTP サーバー）は ritsu-testkit のものを使う。ほかの言語は、出す側のクレート（rulec、koyomi、chobo、geas、dandori、sakai）を dev-dependency に持ち、`ritsu yuen` と同じにつないで、同じプロセスの中で走らせる（ritsu の DESIGN 3.3。SKIP しない）。geas の記録は、`geas map` で一度だけ取ってテストの材料に置いた（`tests/fixtures/geas`）。テストのための外のツール：xmllint（`YUEN_XMLLINT`、無ければ PATH）と `tools/reqif/fetch.sh` が取る 24 個のスキーマ（`YUEN_REQIF_XSD`、無ければ `tools/reqif/xsd`。無ければ SKIP）、curl（`source fetch` と `outdated` のテスト。無ければ SKIP）、`tools/requirements.txt` の `prov==3.2.2` と `reqif==0.1.0`（`uv venv --python 3.13 tools/.venv` に入れる。`YUEN_PYTHON` でほかの場所も使える。無ければ SKIP）、Chrome（`doc` の HTML の画面。`YUEN_CHROME`、無ければ macOS の Google Chrome、PATH の `google-chrome` か `chromium`。無ければ SKIP）。
- 文書は `docs/`（英語の `reference.md`、`yuen explain --all --format markdown` の出力そのものの `codes.md` と `codes.ja.md`）、README.md、README.ja.md、`skills/yuen`（`SKILL.md` は手で書き、ほかは `skills/sync.sh` が `docs/` からコピーする）。README とスキルに載せた `.req` の行、コマンドの出力、診断は、テストが実物と照らし合わせる。

モジュールの分け方と、各段階の作業は PLAN.md にある。

### 16.1 ritsu の土台へ移したもの

yuen は ritsu（七つの言語を一つにまとめる処理系）に取り込まれ、ほかの言語と重なっていたコードを、ritsu の土台のクレート（ritsu-base と ritsu-testkit）のものに替えた（ritsu の PLAN の C.7）。替えたのは、SHA-256、base64、二つの言語の文（`tr!`、`Text`、`Lang`）、診断の共通の部分、台帳の書き出しと再現の走らせ方、コマンドの表の読み方と `--help` の組み立て、参照を読む仕組み（2 章の決まりそのもの）、ルートの探し方と表示のパス、法令のコピーの扱い（引用からコピーのファイルの名前を作ること、コピーの本文、固定の行の書き換え、e-Gov と eCFR への問い合わせ）、テストの共通の部分である。yuen に残したのは、`.req` の字句と構文、診断の yuen の部分（つながり、差分、候補。`Trail`）、参照の診断のコードと文（E011、E012、E013）、台帳とコマンドの表の中身、ReqIF と PROV の書き出しである。参照を試す表は ritsu-base の `tests/fixtures/naming.tsv` 一つになり、yuen が持っていたコピーは消した。

出力は、次のものを除いて一字も変えていない。

- **書き出しの識別子**：12 章に書いたとおり、SHA-256 を一度だけかけた値の先頭 32 桁にした。実装は、ダイジェストをもう一度 SHA-256 にかけていた（`hex(digest(b))` が、渡したバイト列のダイジェストを書くため）。公開する前で、yurai から yuen への改名で識別子はすでに全部変わっていたので、直すのはいまがいちばん安い。ReqIF と PROV の golden（三つの例の三つずつ、九つのファイル）を取り直し、識別子の値のほかは一字も変わらないこと、古い識別子と新しい識別子が一対一に対応すること（150 個）を確かめた。12 章と 13 章の例も取り直した。
- **言語の選び方に `RITSU_LANG` が入った**：`--lang`、`YUEN_LANG`、`RITSU_LANG`、英語の順に読む。`--lang` の説明と `yuen --help` の最後の行が、この順を書くようになった。
- **行の無い診断の JSON**：`line` と `col` を `0` でなく `null` で書く。行が無いことを行番号の 0 で表さないためで、ritsu のどの言語も同じに書く（いまの yuen が JSON で出す診断には、行の無いものは無い）。
- **表示のパス**：最初に渡したパスが絶対パスのとき、コピーや成果物のファイルを絶対パスで書く（2.2）。ディレクトリを歩くときに飛ばす名前に、ritsu のどの言語とも同じく `site-packages` と `__pycache__` が入った。
- **通信**：curl に `--compressed` が付いた（eCFR は付けないと 406 を返す。rulec が見つけたこと）。e-Gov が JSON でないページを返したときは、二度まで取り直してから止める。JSON として読めないときの文の後ろの部分（読めなかった理由）は、ritsu-base の JSON の読み手の言い方になった。
- テストの SKIP の行は、ritsu のどのクレートとも同じ `SKIP: yuen: <理由>` の形になった。本物の e-Gov と eCFR には、`YUEN_NET=1` のときのほか、`RITSU_TEST_LEVEL=platforms` のときも問い合わせる。

ritsu の段階 D の最初の部分で、ritsu の口（ritsu の DESIGN 3.2）に答える `src/ports.rs` を足した。`.req` が持つもの（要件と出典）を、要件はその端の中身（4.1。人がリンクを確かめるときに見るバイト列）を定義の文として、出典は固定した条ごとの `from` の行の形で渡す。要件は名前で指すので、一つのファイルに同じ要件の版が二つあれば、大きい版を渡す。外を指すものは、`satisfied by` と `verified by` の先、`scope` の先、借りた出典と `file` の出典である。端の中身は `yuen check` が計算するものと同じで、テストの材料の記録にあるハッシュ（`起算日` の `a9ebc73907faddc8`、`満了日` の `465b83ed8c251406`）と同じになる（`tests/ports.rs`）。コマンドの振る舞いは変えていない（527 回の出力が一字も違わない）。

ritsu の D.7 で、一式の読み込みを口で作った（3 章）。そのとき土台へ移したのは、統一形式の差分の読み手（geas の `src/diff.rs` の読む部分を ritsu-base の `udiff` にし、geas と yuen が同じものを使う）と、台帳の退いたコードの書き方（ritsu-base の `Repro::Retired`）である。yuen の口（`src/ports.rs` の `Engine`）は、つないだ言語を持つようになり（`Engine::with`）、借りた出典から来た要件にも定義の文を渡す（つないでいなければ、空の文を渡す）。

この変更で、yuen のコマンドの出力が変わったのは次のところである（テストの材料と変異の全部の 1,026 回の出力を、替える前の yuen のバイナリと、替えたあとの `ritsu yuen` で比べた）。

- ほかの言語のものを指すプロジェクト：替える前は「まだ読めない」と言って exit 2 で止まっていたものを、読んで検査する（新しいテストの材料七つと、新しい変異六つ。173 回）。
- `yuen --help`、引数の無い `yuen`（`affected` が表に入った）、`yuen affected --help`（コマンドが無いと言っていた）、`yuen check --help` と `yuen source --help`（exit 2 の説明。まだ読めない成果物と借りた出典が、つながっていない言語のものになった）。
- `yuen explain --all`（英語と日本語、テキストと Markdown）：E106、E107、E202、E203、E205 に再現が付き、E203 の意味が替わり、E204 と W201 が退いた。
- `yuen trace --format json`：リンクごとに `pins` が付き、成果物から始めたときに `pins`、出典の条から始めたときに `pinned_by` が付いた（9 章。リンクが規則やカレンダーを指さなければ空の並び）。72 回。
- `check` の日本語の要約で、範囲の種類の語（`date` など）の前に空白を置く（「範囲の date 3 個」）。替える前のテストの材料には、種類を書いた範囲が無かった。
- E202 の文は、名前を参照と同じ書き方で書く（空白を含む名前は `"…"` で囲む）。E303 の差分の見出しは、そのもの一つの端なら参照を書く（ファイル全体の端なら、これまでどおりパス）。どちらも、替える前のテストの材料では出なかった。

段階 E の最初の部分（ritsu の PLAN の E.1、E.2）で、ほかの言語のものの中身を、言語ごとにではなく、プロジェクトの索引で引くようにした（3.1）。`ritsu check` のために、`yuen::ports::Engine::checked` が、`yuen check` が印字するもの（診断一つずつのテキストと `--format json` のオブジェクト、要約の行）を、コマンドと同じ関数で作って渡す（ritsu の DESIGN 8.3）。コマンドの振る舞いは変えていない（例と fixture の全部の 1,026 回の出力が、クレートのバイナリでも `ritsu yuen` でも一字も違わない）。

同じ部分で、言語がつながっていないときに出すものを、コードのある診断（E206）と exit 2 にした（3.1）。出力が変わるのは、yuen のクレートのバイナリが、ほかの言語のものを指すプロジェクトを渡されたときだけである（標準エラーの一行の文が、標準出力の診断と要約の行になる。`api`・`export`・`trace`・`affected` は、これまでどおり標準エラーに出す）。`ritsu yuen` の出力は変わらない。

### 16.2 英語の材料と日本語の材料

テストの材料、変異、golden、文書の例は、英語を先にし、日本語のものは名前も中身もそのまま横に置く（ritsu の決まり）。yuen では、次のとおりにした。

- **変異**：`tests/mutants/<コード>_<名前>/` の名前が英語のものは、ファイルの名前も中身も英語である。名前が日本語のもの（65）はそのままで、対の英語のもの（65）を足した。名前が ASCII でも中身が日本語だった `E011_dir` は日本語の側に数え、英語の対は `E011_scope_directory` である（合わせて 66 組）。`E206_rulec-not-joined` は、もとから英語である。対は診断のコードの頭でそろえ、`tests/english_mutants.rs` の `PAIRS` が書き、英語のものに日本語の文字が無いこと、対が同じコードを同じ順で出すことを確かめる。日本語の変異を足して対を足さなければ、そのテストが落ちる。golden（`tests/golden/<変異の名前>.en.txt` と `.ja.txt`）は、英語の変異にも日本語の変異にも、英語の出力と日本語の出力を一つずつ持つ。
- **テストの材料**：`tests/fixtures/` の日本語のもの（`period`、`payment`、`rulec`、`koyomi`、`chobo`、`proto`、`dandori`、`sakai`）に、英語のもの（`period_of_months`、`payment_policy`、`fee_rules`、`calendar_sources`、`refunds_book`、`warehouse_proto`、`delivery_flow`、`ordering_terms`）を足した。法令は eCFR（37 CFR 1 の §1.6、1.7、1.8、1.10、1.17、1.27）で、`yuen source fetch` が取った本物のコピーを固定している。確かめた記録（`reviewed/`）は `yuen review` が書いた。`ecfr`（29 CFR 1910.157）と `geas` は、もとから英語である。
- **テスト**：日本語のテストは名前も本体もそのままで、同じ振る舞いを英語の材料で確かめる対のテスト（名前の終わりが `_in_english`）を `tests/english*.rs` に足した。材料を順に歩くテスト（`mutants.rs`、`export.rs` の `PROJECTS`、`codes.rs`）は、英語の材料も歩く。golden は材料の名前ごとにあるので、英語のものは別の名前で足した（`period_cal`、`chobo-refund.json` など）。
- **日本語でしか確かめられないもの**：e-Gov の条の漢数字と、項・号・別表・附則のコピーのファイルの名前（`copies.rs` の `the_files_articles_are_copied_into`）、e-Gov の版の ID と施行日を使う問い合わせ（`fetch.rs` の e-Gov の流れ）、koyomi が固定した条の話（E106 の「カレンダーが固定していない条」、142 条の改正が日付の端に届く範囲を見る `suite.rs` の `an_article_taken_into_a_calendar_marks_only_what_reads_it`）には、英語の対を持たない。eCFR の節には漢数字も版の ID も無く、koyomi が固定できる法令は e-Gov のものだけだからである。コピーの固定と本文（`the_pins_of_the_copies_of_the_civil_code`、`the_text_of_a_copy`）、問い合わせの流れ、借りた出典の端と印の付く範囲は、eCFR の節と、カレンダーが固定する英国の祝日の JSON で、同じことを確かめる対がある。
- **`explain` の再現**：6.2 のとおり、コードごとに英語と日本語の二つを持つ。

## 17. 捨てたもの

各節に書いたもののほかに、全体にかかわるものを挙げる。

- **要件の文の意味を確かめること**（言葉の解析、モデルに読ませる判定）。P1。意味を判定するツールの結果は、同じ入力で同じにならないことがあり、確かめたときから変わったかを言うツールの性質と合わない。
- **主張を走らせること**。`check` は geas の主張も、テストも走らせない（P4）。主張が通るかは、それぞれのツールの仕事である。yuen は、どの主張が要件を確かめているかと、その主張が確かめたときのままかを言う。
- **人が版の数を上げて、古いリンクを無効にすること**（OpenFastTrace）。上げ忘れれば、変わったのに黙る。yuen はハッシュで決め、上げ忘れが起きない。
- **法令を丸ごと一つのハッシュで固定すること**。関係の無い条の改正で、全部の要件に印が付き、人が中身を読まずに確かめ直すようになる（rulec の §15.68 と同じ理由）。条ごとに固定する。
- **コードのタグ**（LOBSTER、OpenFastTrace、StrictDoc の `@relation`）。5.3。
- **要件のサーバーやデータベース、編集の画面**（Doorstop と StrictDoc の Web の画面）。`.req` はテキストで、git とプルリクエストで読み、`doc` が読む人のページを出す。
- **`.rule`・`.cal`・`.book`・`.flow`・`.geas`・`.ctx` を yuen が読み解くこと**。P2。言語の口が渡さないことは、その言語に渡してもらう（3.5）。

## 18. まだやらないこと

どれも、作る理由が見えたら作る。ここに書くのは、黙って消えたように見せないためである。

言語：

- `file` の出典の中の箇所（条、段落、表）を引くこと（1.4）。形の決まった文書のうち、OpenSpec の仕様は 20 章で要件ごとに読めるようにした。
- 利用者が型の付いたフィールドを足すこと（1.5）。
- 書き方をそろえる `yuen fmt`。

一式とのつなぎ方（3.5 の提案に、言語が応えたら）：

- rulec の表・節・行と、koyomi の日付・条件ごとの引用から、出典の辺を成果物の一つ一つに細かくすること（端は ritsu の D.7 で一つ一つになった）。
- geas の記録の古さの見分け（記録の一行めの spec のハッシュ）。
- 成果物のファイルが固定しているのに、どの要件も引いていない条について、`source outdated` が改正を言うこと（いまはその言語自身の `source outdated` の仕事として残す）。
- ReqIF に、成果物のファイルが固定している条を書くこと（12 章。PROV には書く）。

出典のコマンド：

- GitHub のコミットを指す `file` の出典の URL について、そのパスを変えたコミットを並べること（rulec の §15.76）。いまはバイト列のハッシュを比べるだけである。

ページと書き出し：

- `doc` の来歴の図（10 章）。
- ReqIF を読んで `.req` にすること。ReqIF の ZIP（`.reqifz`）。
- PROV-O（13 章）。

公開：

- 日本語の `docs/reference.md`。今は英語だけで、日本語は README.ja.md と `docs/codes.ja.md` にある。
- ドキュメントのサイト、MCP サーバー、ブラウザで試すページ（rulec と dandori にはある）。
- yuen だけのリリースとバイナリ、crates.io。yuen は ritsu の一部として配り、入れ方は ritsu と同じ `cargo install --git https://github.com/i2y/ritsu --locked ritsu` である（コマンドは `ritsu yuen …`、リンクの名前 `yuen` でも呼べる）。

## 19. 確かめたこと（2026-10-03 と 2026-10-04、macOS arm64）

設計を決めるために、この機械で実際に走らせたこと。ツールのバージョンは、rulec 0.22.1（作業場所にコピーしたもの）、koyomi 0.1.0、chobo 0.1.0、geas 0.0.1（どれも `cargo install --locked --path` で作業場所に入れたもの）、xmllint（libxml 2.9.13）、Python 3.13.11 の venv の `prov` 3.2.2 と `reqif` 0.1.0。

**`source_sha256` はファイルのバイト列のハッシュ**。`rulec api` の `印紙税の本則と軽減.rule`（`dc176eebd83f26e3…`）、`koyomi api` の `民法の期間.cal`（`c9b94eecde23e6b5…`）、`chobo api` の `refunds.ja.book`（`b2daf81c87ff979e…`）は、三つとも `shasum -a 256` の値と同じだった。

**行ごとの引用は、どの JSON にも無い**。`rulec api` の最上位のキーは `rule`、`alias`、`version`、`source_sha256`、`preconditions`、`applies`、`projection`、`machine`、`sources` と出力先ごとの項で、`sources` は出典ごとの固定（`pins`）だけを持つ。`rulec graph` の `nodes[].by[]` は表の名前と行の数と `overrides`、`define` の式を持つが、引用を持たない。`rulec certificate` のキーを全部たどっても、`第91条` も `別表` も出てこない（表の行は `cells`、`tests`、`origin`、`line`、`source` と証拠のキーを持つ）。`koyomi api` は `dates[]` に `name`、`alias`、`params` だけを持ち、操作の行も引用も無い。

**検査を通らないファイルには api が出ない**。`koyomi api 民法の期間_読み方の比較.cal` は exit 1、rulec の変異 `m_e038.rule`（コピーが固定と違う）への `rulec api` は exit 1 で `error[E038]: Fragment 第91条 of source 措置法 has changed` を出した。

**読み方を変えた `.cal` は koyomi の検査を通る**。`民法の期間.cal` の `満了日_142条` の `if closed + 1 day` を `roll following` に替えたものを作業場所で検査すると、`民法の期間.cal: ok — 4 claims hold on all 4,380 combinations of 起点 (2026-01-01..2026-12-31) and 月数 (1..12)` で、`source_sha256` の先頭 16 桁は `553a87c58fb264e0` になった（元は `c9b94eecde23e6b5`）。

**chobo の振替は定義のハッシュを持つ**。`chobo api` の `transfers[]` は `name`、`description`、`params`、`key`、`pending`、`moves`、`code`、`definition`（32 桁）、`operations` を持ち、`ids.definition` は「the transfer kind written the one way」と「each account kind its moves touch, written the one way」と説明している。

**geas の記録と affected**。greeter の例を作業場所にコピーして `geas map greeter.geas` を走らせると、記録の一行めは次のとおりだった。

```json
{"geas_map":1,"spec":"greeter.geas","root":".","claims":[{"name":"greets by name","status":"ok","targets":["api"]},{"name":"rejects an empty name","status":"ok","targets":["api"]},{"name":"totals accumulate across requests","status":"ok","targets":["api"]},{"name":"unknown paths are 404","status":"ok","targets":["api"]}]}
```

`server.py` の、空の名前を受け付けないときの文言を一行変えた差分に、変更の前と後の二つの記録を渡した `geas affected greeter.geas change.diff --map … --map … --json` は、次を出して exit 0 だった。記録が変更の前のものだけのときは、E063 で記録を足すよう言って exit 2 だった。

```json
{"geas":1,"spec":"greeter.geas","diff":"../change.diff","records":[{"file":".geas/greeter.map.jsonl","side":"before"},{"file":"../after.jsonl","side":"after"}],"claims":[{"index":2,"name":"rejects an empty name","status":"ok","lines":[{"file":"server.py","side":"after","lines":"26","how":"ran","startup":null},{"file":"server.py","side":"before","lines":"26","how":"ran","startup":null}]}],"unclaimed":[],"deleted":[],"outside":[],"spec_changed":[],"baseline_changed":false,"ok":true}
```

**ReqIF の検証**。`reqif.xsd`（SHA-256 `9243f345540f25db…`）が読み込むスキーマを全部たどると 24 個のファイルになった（OMG の二つ、W3C の `xml.xsd` が二つ、XHTML のモジュールが 20）。カタログで手元に向けた xmllint の結果：

```
sample.reqif validates
broken2.reqif:33: element SPEC-OBJECT: Schemas validity error : Element '{http://www.omg.org/spec/ReqIF/20110401/reqif.xsd}SPEC-OBJECT': The attribute 'LAST-CHANGE' is required but missing.
broken2.reqif fails to validate
broken.reqif validates
```

（`broken.reqif` は、つながりの `TARGET` を無い要件 `r-9` に向けたもの。）同じ三つに `reqif validate` を当てると、`sample.reqif` と `broken2.reqif` は exit 0、`broken.reqif` は `A <SPEC-RELATION>'s <TARGET> contains a link to a non-existing <SPEC-OBJECT>: r-9` で exit 1 だった。

**PROV の読み書き**。手で書いた PROV-JSON は、`hadPrimarySource` の項を持たせると `prov` が読まず（`'hadPrimarySource' is not a recognised PROV-N record-type keyword`）、`wasDerivedFrom` に `prov:type='prov:PrimarySource'` を付けた形に直すと 8 件の記録として読めた。`prov` が書いた PROV-N を `prov` で読み直すと、元の文書と等しかった。

**端のハッシュの試作**。4 章の定義どおりに組んだ Python の試作で計算した値：要件 `起算日` の端 `a9ebc73907faddc8`、`満了日` の端 `465b83ed8c251406`、`満了日_142条` の端 `d4f2d2a67322df17`。koyomi の条件 `142条の満了日は満了日以後` の端 `825aa6c6f7ccf314`（ほかの三つの条件は `447d80ca751bd681`、`2a8e130e4c527692`、`0b951fef68a36592`）。chobo の振替 `返金` の端 `84e9ce254075c697`（1,206 バイト）、`売上計上` の端 `851ab806078168fe`、勘定 `返金できる残り` の端 `9f9b0d74872f62a4`、`売上` と `返金済み` の端はどちらも `35a4ec5a2ee5eb06`。

**似たものの版**。Doorstop v3.2（2026-07-10）、StrictDoc 0.30.1（2026-09-16）、TRLC 3.1.0（2026-09-30）、LOBSTER 1.0.6（2026-07-30）、OpenFastTrace 4.10.0（2026-09-20）、`reqif` 0.1.0（2026-08-11）。どれも GitHub の最新のリリースで確かめた。Doorstop の stamp の作り方は、リポジトリの `develop` の枝の `doorstop/core/item.py` と `types.py` を読んだ。ReqIF の実装ガイドは、prostep ivip が配る v1.10 の PDF を読んだ。

**段階 C で確かめたこと（書き出しと出典のコマンド）**。同じ機械で、xmllint（libxml 2.9.13）、Python 3.13.11 の venv の `prov` 3.2.2 と `reqif` 0.1.0、curl で確かめた。

- `tools/reqif/fetch.sh` が取った 24 個のファイルは、どれも A の段階に記録した SHA-256 と同じだった。
- xmllint のスキーマの検証は、`LOCAL-REF`（`xsd:IDREF`）が無い識別子を指していても通した。手で書いた文書の `SPEC-OBJECT-TYPE-REF` を無い識別子に向けると、`validates` と言った。`reqif validate` も、この文書を通した。だから参照の宛先は、yuen のテストが自分で、すべての `-REF` について確かめる（12 章）。
- `reqif validate --use-reqif-schema` は、通信せずに動き、`LAST-CHANGE` を消した文書を「1 schema issues found」と言って exit 1 で落とした。
- `prov` 3.2.2 は、PROV-N を読める（`ProvDocument.deserialize(…, format="provn", profile="strict")`）。修飾名のローカル部に日本語を書いても（`y:role/法務`）読み、PROV-JSON の `_:` で始まる名前の関係の記録を、識別子の無い記録として読んだ。
- yuen が書き出した `period`、`ecfr`、`payment` の ReqIF は、xmllint、`reqif validate`、`reqif validate --use-reqif-schema` のどれでも通り、PROV は、PROV-N と PROV-JSON が `prov` で等しい文書として読めた（12 章と 13 章のテスト）。
- 2026-10-03 10:38（日本時間）に、本物の e-Gov に `yuen source outdated tests/fixtures/period` を一度問い合わせた。`law_revisions` は 2026-10-01 より後に施行される版を五つ返し、どの版でも民法 140〜143 条の本文は変わらず、exit 0 だった（14 章）。版の ID は `129AC0000000089_20270623_508AC0000000045`、`129AC0000000089_20271205_507AC0000000057`、`129AC0000000089_20280613_505AC0000000053`、`129AC0000000089_20281223_508AC0000000045`、`129AC0000000089_20290623_508AC0000000045` だった。本物の eCFR には問い合わせていない。

**ritsu の D.7 で確かめたこと（一式の読み込み、2026-10-04）**。ほかの言語は ritsu のワークスペースの中のもの（それぞれの言語のクレートの `Engine` が口に答える）、xmllint（libxml 2.9.13）、Python 3.13.11 の venv の `prov` 3.2.2 と `reqif` 0.1.0、geas の記録を取るときの python3 3.14.6。

- テストの材料を七つ足した。`tests/fixtures/rulec`（rulec のコーパスの `印紙税の本則と軽減.rule` と `osha_extinguisher.rule` と、その出典のコピー。要件の側にも同じバイト列のコピーを置いた）、`tests/fixtures/koyomi`（koyomi の例の `民法の期間.cal`、`支払_20日締め翌月10日払い.cal` とカレンダーと、出典のコピー。出典は借りる）、`tests/fixtures/chobo`（chobo の例の `refunds.ja.book`）、`tests/fixtures/geas`（geas の例 greeter と、`geas map greeter/greeter.geas --root .` で取った変更の前と後の記録と、その差分）、`tests/fixtures/proto`（dandori の例の二つの `.proto`）、`tests/fixtures/dandori`（dandori の例 `arrange_delivery.ja.flow`）、`tests/fixtures/sakai`（sakai の例の `受注.ctx`）。確かめた記録は `ritsu yuen review` で書いた。
- geas の記録：`geas map` は四つの主張を走らせて全部 ok で、`map: 2 files · 40 lines of code, 39 run by some claim` と言った。記録の一行めは `{"geas_map":1,"spec":"greeter/greeter.geas","root":"..",…}` で、`server_refactored.py` の `code` は null（どのランタイムも報告しない）だった。
- 借りた出典で書いた `民法の期間.req` の要件の端は、出典を自分で保存した `tests/fixtures/period` と同じだった（`起算日` `a9ebc73907faddc8`、`満了日` `465b83ed8c251406`、`満了日_142条` `d4f2d2a67322df17`）。
- ファイルを指したときの端は、A の段階に `source_sha256` で見た値のままだった（`印紙税の本則と軽減.rule` `dc176eebd83f26e3`、`osha_extinguisher.rule` `a52e955b88af88d6`）。rulec の固定は `法 別表第一` `0ba69792e960021e`、`措置法 第91条` `85faf53f6f6e8196`、`osha "§1910.157"` `c2a9ce966c7e2269` だった。
- そのもの一つの端（取り直したハッシュ）：

  | 成果物 | 端 |
  |---|---|
  | `rulec "rules/印紙税の本則と軽減.rule" table 本則` | `21432c19a67fe72e` |
  | `… clause 非課税` | `5bc61581e0c31022` |
  | `… table 軽減` | `4ba4c2dec2c8262b` |
  | `… define 軽減期間` | `b5b30cefd66e58d6` |
  | `rulec "rules/osha_extinguisher.rule" table distance` | `5681500476af8a56` |
  | `koyomi "民法の期間.cal" date 起算日` | `15aa6c91d6aaae80` |
  | `… date 満了日` | `23441fd408f07548` |
  | `… date 満了日_142条` | `bba4761410179e6e` |
  | `… claim 142条の満了日は満了日以後` | `7c616f5dcc9503a8` |
  | `… claim 満了日は単調` | `53b5eb9afcef2219` |
  | `… claim 142条の満了日も単調` | `44569913a3f84e29` |
  | `… claim 満了日は起点より後` | `5e7bb0ed51835dc7` |
  | `koyomi "支払_20日締め翌月10日払い.cal" date 支払日` | `feb9535fb9350ecc` |
  | `… claim 営業日に払う` | `3cf8fc3fb2dda9d9` |
  | `chobo "refunds.ja.book" transfer 返金` | `84e9ce254075c697`（1,206 バイト。A の段階と同じ） |
  | `… transfer 売上計上` | `851ab806078168fe`（同じ） |
  | `… account 返金できる残り` | `9f9b0d74872f62a4`（同じ） |
  | `… account 売上`、`… account 返金済み` | どちらも `35a4ec5a2ee5eb06`（同じ） |
  | `geas "greeter/greeter.geas" claim "greets by name"` | `2b4e11c2cc191431` |
  | `… claim "rejects an empty name"` | `3f7c8d1b2bbcc309` |
  | `… claim "totals accumulate across requests"` | `d653be1c06f5c8b1` |
  | `… claim "unknown paths are 404"` | `1950201e8644bc57` |
  | `proto "warehouse.proto" service StockService method Reserve` | `cadb2fc727e9af81` |
  | `… service StockService method Release` | `2a0912a66889e515` |
  | `… enum Stock` | `d7450e7503c905f7` |
  | `proto "fulfillment.proto" service FulfillmentService method Fulfill` | `6c73e17bf5bedba7` |
  | `… message FulfillResponse field tracking_number` | `fb7f7e2617539e54` |
  | `dandori "arrange_delivery.ja.flow" task 翌日便を頼む` | `3c19313a3f633021` |
  | `… task 通常便を頼む` | `fb518dde20b90816` |
  | `… record 集荷 field 追跡番号`、`… output 追跡番号` | どちらも `69aa90b09002e1fc` |
  | `sakai "contexts/受注.ctx" term 注文` | `1b2bb352a123c296` |
  | `… term キャンセル` | `dbfd211b7e4cef4b` |

  koyomi の条件の端は、A の段階の試作では `koyomi api` の `claims[]` の一つから `name` を除いた JSON で（`142条の満了日は満了日以後` が `825aa6c6f7ccf314`、ほかの三つが `447d80ca751bd681`、`2a8e130e4c527692`、`0b951fef68a36592`）、koyomi が渡す定義の文（条件の行）に替わって、上の値になった。日付の端は、A の段階ではファイル全体（`c9b94eecde23e6b5`）だった。chobo の端は、A の段階と同じ形の JSON を chobo の口が渡すので、同じ値になった。

## 20. OpenSpec の仕様を出典にする

2026-10-05 に足した。OpenSpec（Fission-AI の `@fission-ai/openspec`。この章は 2026-09-30 に出た 1.14.0 の文書とソースを読み、手元に入れて走らせて確かめた）は、仕様から始める開発の道具で、仕様（`openspec/specs/<capability>/spec.md`）と変更の提案（`openspec/changes/<id>/`）を Markdown で持つ。仕様は `## Requirements` の下に `### Requirement: <名前>` の要件を並べ、要件ごとに `#### Scenario: <名前>` のシナリオを GIVEN／WHEN／THEN の箇条書きで書く。変更の提案は `proposal.md`、`tasks.md` と仕様の差分（`specs/<capability>/spec.md` の `## ADDED|MODIFIED|REMOVED|RENAMED Requirements`）を持ち、`openspec archive` が差分を仕様に当てて、提案を `changes/archive/` へ移す。

### 20.1 何をつなぐと価値があるか

OpenSpec は、人とエージェントが「何をするか」に合意するところまでを受け持つ。合意したあとの三つは、OpenSpec の外にある。

- 実装が仕様に従ったか。OpenSpec の `/opsx:verify` はエージェントに読ませて確かめるもので、同じ入力で同じ答えになるとは限らない。
- 仕様の要件が変わったとき（MODIFIED を含む変更を archive したとき）、見直すべき規則・フロー・コード・主張はどれか。OpenSpec は、要件から先の成果物を知らない。
- 要件が何から来て、誰がいつ決め、誰が承認すべきか。`proposal.md` に理由の文章はあるが、ハッシュで固定した確かめの記録は無い。

yuen はこの二つめと三つめに当たる。要件を出典の条から読み、満たすものと確かめるものにつなぎ、どこかが変われば止める。一つめは geas が受け持つ（シナリオと主張。geas の DESIGN §17）。

**決定**：OpenSpec の仕様を、法令と同じく、条（ここでは要件）ごとに固定して読む出典の種類にする。仕様が変われば、その要件を引く `from` のリンクと、その先の `satisfied by`・`verified by` のリンクに印が付き、確かめたときのブロックとの差分を見せる。まだ archive していない変更の提案が、固定した要件をどう変えるかは、`yuen source outdated` と `yuen affected` が答える。

### 20.2 書き方

```
source greeting = openspec "openspec/specs/greeting/spec.md"
  "Greeting by name" sha256:…
  "Running total" sha256:…

requirement greets_by_name
  text "A greeting names whoever asked for it"
  owner api
  from @greeting "Greeting by name"
```

- `source <名前> = openspec "<spec.md のパス>"`。パスは `file` の出典と同じく `.req` のディレクトリからの相対で、絶対パスとルートの外は E013。
- 下の行に、引く要件ごとの固定を書く（法令の条の固定と同じ形）。名前は OpenSpec の archive が MODIFIED・REMOVED・RENAMED の見出しと突き合わせる名前で、`Requirement:` のあとの文字列から、見出しの末尾の `#` の並びを除いて前後の空白を落としたもの。大文字と小文字を区別し、書いたとおりに比べる。空白を含むので、ふつうは `"…"` で囲む。
- 引用は `from @greeting "Greeting by name"`。仕様を丸ごと引くこと（`from @greeting`）はできない（E105）。

`openspec` は出典の行の語で、名前には使える（`law`、`file` と同じ）。

### 20.3 要件の端

**決定**：OpenSpec の要件の端の中身は、その要件のブロックである。見出しの行から、次の要件の見出しか `## ` の行の手前までで、末尾の空白（改行を含む）を落とし、行を LF でつないだもの。CR LF は LF として読み、先頭の BOM は落とす。コードブロック（``` か ~~~）の中の行は、見出しとして読まない。ハッシュは、ほかの端と同じく、その UTF-8 のバイト列の SHA-256 の先頭 16 桁である。要件の端（4.1）の行は `from openspec <ルートからのパス> <要件の名前> sha256:<ハッシュ>` になる。

この決まりは OpenSpec 自身の読み方（ソースの `src/core/parsers/requirement-blocks.ts` の `extractRequirementsSection`）と同じで、archive が MODIFIED の差分で置き換えるのはちょうどこのブロックである。だから、要件を変える変更を archive すれば、その要件の端は必ず変わり、ほかの要件を変える変更や、`## Purpose` の書き直しでは変わらない。読み手は ritsu の土台（`ritsu_base::openspec`。geas もシナリオを読むのに使う）に置き、OpenSpec 1.14.0 の読み手が同じファイルから作るブロックとシナリオの名前（`expected.json`。npm で入れた OpenSpec を node で呼んで作った）と、一字も違わないことをテストが確かめる。

**理由**：

- 端をファイル全体にすると、仕様のどこを直しても、その仕様を引くすべての要件に印が付く（法令を丸ごと固定しない理由と同じ。17 章）。
- シナリオを端に含めたのは、OpenSpec ではシナリオが要件の受け入れの条件だからである。30 分を 15 分に変えるのがシナリオの行だけでも、満たすものと確かめるものは見直すべきである。MODIFIED がブロックを丸ごと置き換えることとも合う。
- 空白を詰めるなどの、yuen だけの正規化はしない。人が読んだのは、OpenSpec が archive で書き、`openspec show` が見せるブロックそのものである。

**捨てたもの**：

- 要件の本文（`openspec show --json` の `text`）だけを端にすること。シナリオの変更を見落とす。
- `openspec` の CLI を子プロセスで呼び、`show --json` を読むこと。`check` が Node と OpenSpec を要るようになり、通信しない・何も走らせないという `check` の前提（P4）から外れる。OpenSpec の Markdown は形が決まっていて、読み方はソースに書いてある。その読み方どおりの読み手を土台に書き、OpenSpec の読み手の結果と照らし合わせるほうが確かである。
- `file` の出典で仕様を丸ごと固定すること。これはいまでも書けるが、要件ごとの差分も、変更の提案の読み方も無い。

### 20.4 検査

`check` の 3 の段（出典）で、次を確かめる。

| 状態 | 診断 |
|---|---|
| 仕様のファイルが無い | E101 |
| 仕様として読めない（UTF-8 でない、`## Requirements` の節が無い、同じ名前の要件が二つある） | E104。変更の差分のファイルを渡したなら、そう言う |
| 固定した名前の要件が仕様に無い | E108（新しいコード）。大文字と小文字や空白だけが違う名前があれば、それを候補に挙げる |
| 引いている要件に固定が無い、固定の行に `sha256:` が無い | E102 |
| 要件のブロックが固定と違う | E103。確かめたときのブロックが `reviewed/` にあれば、差分を見せる |
| 固定した要件を、どの要件も引いていない | W101 |
| 仕様の要件のうち、固定していないものがある | W102（新しいコード） |
| 引いた要件のシナリオに、同じ名前の主張が、要件を確かめる geas の主張の中に無い（7 の段） | W402（新しいコード） |

W102 は、OpenSpec の仕様に書いた要件を、プロジェクトの要件が一つも読んでいないことを言う。一部だけを読むと決めたなら、外す要件も yuen の要件として引き、`not satisfied` と `not verified` に理由を書いて承認を得る。外したことが記録に残り、理由を読むのは持ち主になる（1.7 の見送りと同じ考え）。仕様全体を範囲（1.8）のように宣言する書き方は作らなかった。範囲は満たす側の成果物の集まりで、出典の側に同じ語を使うと、意味が二つになる。

7 の段（カバレッジ）では、OpenSpec の要件を引き、geas の主張で確かめている要件について、その OpenSpec の要件のシナリオのうち、同じ名前の主張が、要件を確かめる主張（`verified by geas … claim …`。spec を丸ごと指したなら、その spec のすべての主張）の中に無いものを、W402（新しいコード）で言う。シナリオと主張は名前で突き合わせる（geas の DESIGN §17 と同じ決まり）。geas で確かめていない要件には言わない。規則の検査や koyomi の条件で確かめる要件では、シナリオと主張を突き合わせる意味が無いからである。変更を archive してシナリオが足されると、固定し直して確かめ直したあとも、この警告が新しいシナリオの主張を求める。

**E103 と、そのあとの E302**：仕様が変わった直後の `check` は、まず E103（固定と違う）で止まり、差分を見せる。`yuen source pin` で固定を書き換えると、その要件を引く `from` のリンクに E302、その先のリンクにも E302 が付き、一本ずつ人が確かめるまで止まる。法令の改正を取り込むときの順（コピーを取る、固定する、リンクを確かめる）と同じで、固定の行は「プロジェクトがこの版の要件を読む」という宣言として、プルリクエストの差分に残る。

**捨てたもの**：固定を置かず、`from` の記録のハッシュだけで仕様の変化を見ること。手順が一つ減るが、出典の書き方が一つだけ違う形になり、`source pin`・`source outdated`・`api` の `pins` がこの種類だけ別の扱いになる。法令と同じ形にそろえた。

### 20.5 変更の提案：`source outdated` と `affected`

**決定**：`yuen source outdated` は、`openspec` の出典について、まだ archive していない変更の提案を読む（通信しない）。仕様のパスが `<dir>/openspec/specs/<capability>/spec.md` の形なら、`<dir>/openspec/changes/` の下の、`archive` を除いた各変更の `specs/<capability>/spec.md` を読み、固定した要件に何をするかを言う。

- MODIFIED：要件のブロックが置き換わる。それを引く要件と持ち主、確かめ直しになるリンクと見送りの数（14 章と同じ数え方）、そのリンクが指す成果物。
- REMOVED：要件が無くなる。引く要件が出どころを失う。
- RENAMED：名前が変わる。固定と引用の名前を直すことになり、ブロックの見出しの行も変わる。
- ADDED：プロジェクトがまだ読んでいない要件が増える（archive すれば W102）。

固定した要件を変える提案があれば exit 1、ADDED だけなら 0 にする。法令の改正が施行される前に言うのと同じく、archive の前に言う。

`yuen affected --diff` は、差分が触るファイルに OpenSpec のものがあれば、二つを足して答える。

1. 出典の仕様のファイル（8 章の 2 の、出典のコピーとして）：ディスクのファイルが差分のどちらの側かを確かめ、もう一方の側を差分から組み立てて、両側の仕様を読む。ブロックが違う要件と、片側にしか無い要件が、差分が触る要件である。そのうち固定している要件を引く要件を挙げる。どちらの側とも合わなければ、その仕様から引いている要件の全部を挙げる。
2. 変更の提案の差分のファイル（`openspec/changes/<id>/specs/<capability>/spec.md`）：上の `source outdated` と同じ読み方で、その変更が固定した要件に何をするかと、触る要件を挙げる。提案を出すプルリクエストの差分に、archive の前に答えられる。

**捨てたもの**：

- 変更の提案を名前で渡すフラグ（`yuen affected --change trim-names`）。`source outdated` が全部の提案を見て、`affected` は差分に入った提案を見るので、二つで足りる。
- archive した提案（`changes/archive/`）を読むこと。当てたあとの仕様は `specs/` にあり、端のハッシュで変化が分かる。

### 20.6 ほかのコマンド

- `source fetch`：取るものは無い。仕様はプロジェクトのファイルで、そこに書くものだからである。その旨を一行で言う。
- `source pin`：固定の行の 16 桁を、いまのブロックのハッシュに書き換える。引いているのに固定の行が無い要件には、行を足す。
- `trace`、`doc`：要件のブロックを、法令の条と同じ場所に引用する。`doc` の出典の節は、仕様の要件ごとに、固定と引く要件を表にする。
- `api`：出典に `"kind": "openspec"`、`path`、`pins` を出す。
- `export`：ReqIF と PROV は、OpenSpec の要件を、パスと要件の名前で区別する出典として書く。属性は `file` の出典と同じ `yuen.file`（PROV は `yuen:file`）に仕様のパスを、名前（ReqIF の `ReqIF.Name`、PROV の `prov:label`）に `<出典> "<要件>"` を、本文（`ReqIF.Text`）にブロックの行を書く。新しい属性は足さなかった。ReqIF は属性の定義を文書の頭にまとめて書くので、一つ足すと、OpenSpec を読まないプロジェクトの書き出しまで変わるからである。
- ritsu の口：yuen の出典の定義の文は、固定ごとの `from openspec …` の行。`References` は、仕様のファイルを出典として指したものとして渡す。

### 20.7 範囲と、まだやらないこと

範囲に入れたもの：`openspec` の出典（書き方、検査、端と印、`review`、`source` の三つのコマンド、`affected`、`trace`、`doc`、`api`、`export`、口）、E108、W102、W402、例（`openspec_greeter` と、変更を archive したあとに止まる `openspec_greeter_archived`）、テストの材料と変異（英語と日本語）、README（英日）、`docs/reference.md`、台帳、スキル。

まだやらないこと：

- 仕様のストア（OpenSpec の stores。ベータ）や、`config.yaml` の `references` が指すほかのリポジトリの仕様を読むこと。いまは同じルートの中のファイルだけを読む。
- シナリオの単位で固定すること。シナリオは要件のブロックに含まれ、シナリオと主張の対応は geas が受け持つ（geas の DESIGN §17）。
- 要件の文（`text`）を OpenSpec の要件から取って省けるようにすること。要件の端は yuen の要件の文から作る（4.1）ので、出典の文を流用すると、リンク元が変わったのか要件が変わったのか（E302 と E303 の区別）がぼやける。
- Spec Kit や Kiro の仕様を同じように読むこと。形が決まっていて、読み方が公開されていれば、同じ形で足せる。

### 20.8 実際の出力

例 `openspec_greeter` は、geas の例 greeter の仕様を OpenSpec で書き（要件三つ、シナリオ四つ。シナリオには geas の主張と同じ名前を付けた）、変更の提案 `trim-names`（名前の前後の空白を除く MODIFIED と、ヘルスチェックの ADDED）を持つ。仕様と提案は、手元に入れた OpenSpec 1.14.0 の `openspec validate --all --json` を通した。`openspec_greeter_archived` は、そのコピーで `openspec archive trim-names --yes` を走らせたあとのもので、仕様の書き換えと提案の移動は OpenSpec が行った。`.req` と `reviewed/` は二つの例で同じバイト列である（`tests/openspec.rs` が確かめる）。

archive の前に、`source outdated` が言うこと（通信しない）：

```
$ yuen source outdated examples/openspec_greeter/greeter.req --root examples/openspec_greeter
greeting: the change trim-names, not yet archived, modifies "Greeting by name" (examples/openspec_greeter/openspec/changes/trim-names/specs/greeting/spec.md:3)
  what changes in the requirement:
      @@ -1,4 +1,4 @@
        ### Requirement: Greeting by name
      - The service SHALL answer `GET /greet?name=<name>` with status 200 and a JSON body whose `message` is `Hello, <name>`, and SHALL refuse an empty name with status 400.
      + The service SHALL answer `GET /greet?name=<name>` with status 200 and a JSON body whose `message` is `Hello, <name>` with the spaces around the name removed, and SHALL refuse a name that is empty once they are removed with status 400.

        #### Scenario: greets by name
      @@ -9,3 +9,7 @@
        #### Scenario: rejects an empty name
        - **WHEN** a client asks for `/greet?name=`
      + - **THEN** the status is 400
      +
      + #### Scenario: rejects a name of spaces
      + - **WHEN** a client asks for `/greet?name=%20%20`
        - **THEN** the status is 400
  cited by: greeting_by_name (owned by api, examples/openspec_greeter/greeter.req:12)
  to look at again: 4 links
  read it; once it is archived, yuen source pin pins the new requirement, and yuen check then marks these
greeting: the change trim-names, not yet archived, adds the requirement "Health check" (examples/openspec_greeter/openspec/changes/trim-names/specs/greeting/spec.md:21), which no requirement of the project reads yet
```

archive のあと、固定し直す前の `check`：

```
$ yuen check examples/openspec_greeter_archived/greeter.req --root examples/openspec_greeter_archived
warning[W102]: examples/openspec_greeter_archived/greeter.req:7:8: A requirement of the spec examples/openspec_greeter_archived/openspec/specs/greeting/spec.md is pinned by no source of the project: "Health check"
     7 | source greeting = openspec "openspec/specs/greeting/spec.md"
  = Pin each one and read it with a requirement. To leave one out, read it with a requirement all the same and write `not satisfied` and `not verified` with the reasons: leaving it out is then on record, with its approval.
error[E103]: examples/openspec_greeter_archived/greeter.req:8:3: greeting "Greeting by name" does not match its pin (pinned sha256:1c3d865f4a521275, the requirement is sha256:228485bfce459383)
     8 |   "Greeting by name" sha256:1c3d865f4a521275
  = The requirement changed in the spec after it was pinned (an archived change, or an edit). Read what changed, then pin it again (`yuen source pin`).
  what changed in the requirement since it was pinned (the spec examples/openspec_greeter_archived/openspec/specs/greeting/spec.md):
      @@ -1,4 +1,4 @@
        ### Requirement: Greeting by name
      - The service SHALL answer `GET /greet?name=<name>` with status 200 and a JSON body whose `message` is `Hello, <name>`, and SHALL refuse an empty name with status 400.
      + The service SHALL answer `GET /greet?name=<name>` with status 200 and a JSON body whose `message` is `Hello, <name>` with the spaces around the name removed, and SHALL refuse a name that is empty once they are removed with status 400.

        #### Scenario: greets by name
      @@ -9,3 +9,7 @@
        #### Scenario: rejects an empty name
        - **WHEN** a client asks for `/greet?name=`
      + - **THEN** the status is 400
      +
      + #### Scenario: rejects a name of spaces
      + - **WHEN** a client asks for `/greet?name=%20%20`
        - **THEN** the status is 400
  = The line, fixed: "Greeting by name" sha256:228485bfce459383
warning[W402]: examples/openspec_greeter_archived/greeter.req:15:3: The scenario "rejects a name of spaces" of greeting "Greeting by name" has no claim of its name among the claims that check greeting_by_name
    15 |   from @greeting "Greeting by name"
  = geas holds a scenario to the claim of its name (`geas scenarios`). Write a claim of that name with the person who reads the claims, and link it with `verified by`; if a claim of another name runs the scenario, ask which of the two names is to change.
examples/openspec_greeter_archived/greeter.req: 1 error, 2 warnings
```

`yuen source pin` で固定し直すと、`from` のリンクに E302 が付き（同じ差分を見せる）、その先の三本（`satisfied by file "server.py"` と二つの主張）にも E302 が付く。ほかの二つの要件のリンクには付かない。人が見て `review` を書けば、残るのは W102（ヘルスチェック）と W402（提案が足したシナリオ「rejects a name of spaces」に、同じ名前の主張がまだ無い）になる（`tests/openspec.rs` の `pinned_again_the_links_below_the_requirement_are_marked`）。

提案のフォルダーを足すプルリクエストの差分（`diffs/propose.diff`）と、archive が仕様に当てた差分（`diffs/archive.diff`）への `affected`：

```
$ yuen affected examples/openspec_greeter/greeter.req --root examples/openspec_greeter --diff examples/openspec_greeter/diffs/propose.diff
diff: examples/openspec_greeter/diffs/propose.diff
OpenSpec changes the diff touches, not yet archived:
  trim-names (examples/openspec_greeter/openspec/changes/trim-names/specs/greeting/spec.md, for examples/openspec_greeter/openspec/specs/greeting/spec.md):
    MODIFIED "Greeting by name": cited by greeting_by_name
    ADDED "Health check": no requirement reads it yet
the claims the change touches (geas "greeter.geas"; records: examples/openspec_greeter/.geas/greeter.map.jsonl (either side)):
  none
changes no requirement reaches: none
other files the diff touches: examples/openspec_greeter/openspec/changes/trim-names/proposal.md, examples/openspec_greeter/openspec/changes/trim-names/tasks.md
requirements touched:
  greeting_by_name (examples/openspec_greeter/greeter.req:12): owner api; from greeting "Greeting by name"
1 requirement touched; ask api
$ yuen affected examples/openspec_greeter/greeter.req --root examples/openspec_greeter --diff examples/openspec_greeter/diffs/archive.diff
diff: examples/openspec_greeter/diffs/archive.diff
copies of sources the diff touches:
  examples/openspec_greeter/openspec/specs/greeting/spec.md (greeting "Greeting by name", "Health check"): cited by greeting_by_name
the claims the change touches (geas "greeter.geas"; records: examples/openspec_greeter/.geas/greeter.map.jsonl (either side)):
  none
changes no requirement reaches: none
requirements touched:
  greeting_by_name (examples/openspec_greeter/greeter.req:12): owner api; from greeting "Greeting by name"
1 requirement touched; ask api
```

二つめでは、差分の両側の仕様を組み立てて比べるので、変わったのは「Greeting by name」と、足された「Health check」だけと分かる。最初は差分の行の番号で要件のブロックを当てていたが、要件の境目に足した行（シナリオを一つ足した行）が次の要件のブロックに数えられ、仕様の三つの要件の全部を挙げてしまった。それで両側を比べる形にした。

### 20.9 確かめたこと（2026-10-05、macOS arm64）

- **版と出どころ**：npm の `@fission-ai/openspec` の最新は 1.14.0（2026-09-30 公開。<https://registry.npmjs.org/@fission-ai/openspec> の `dist-tags.latest`）。`npm install --prefix` で作業場所に入れて走らせ、ソースは <https://github.com/Fission-AI/OpenSpec> のタグ `v1.14.0` を読んだ。読んだ文書は `docs/concepts.md`、`docs/cli.md`、`docs/writing-specs.md`、`docs/agent-contract.md`。読み方の決まりは `src/core/parsers/` の `requirement-blocks.ts`（`extractRequirementsSection`、`parseDeltaSpec`、`normalizeRequirementName`）、`requirement-text.ts`、`code-fence.ts`、`markdown-parser.ts`、`spec-structure.ts`、archive の順は `src/core/specs-apply.ts`（RENAMED、REMOVED、MODIFIED、ADDED）から取った。
- **CLI の JSON**：`openspec show <spec> --type spec --json` の要件は `name`、`text`、`scenarios`（`name` と `rawText`）。`text` は本文だけで、シナリオを含まない（20.3 で端に使わなかった理由）。`list --json`、`validate --all --json`、`archive <id> --yes --json` の形も、`docs/agent-contract.md` のとおりだった。
- **archive が書く仕様**：英語と日本語の例の提案で `openspec archive` を走らせ、MODIFIED の要件のブロックが提案のブロックと一字も違わないこと、ほかの要件のブロックが一字も変わらないこと、ADDED の要件が最後に足されることを確かめた（`tests/openspec.rs` の `the_archived_example_is_the_other_after_the_archive`）。
- **読み手の照らし合わせ**：ritsu-base の読み手が作るブロックとシナリオの名前と差分の四つの節を、OpenSpec 1.14.0 の読み手を node で呼んで作った `expected.json`（コードブロックの中の見出し、見出しの末尾の `#`、CR LF と BOM、`Scenario:` の無い四段の見出し、本文の無いシナリオ、要件でない三段の見出し、`*` と `+` の箇条書きの RENAMED と REMOVED、対の無い `FROM:` を含む）と比べ、一字も違わないことを確かめた（ritsu-base の `tests/openspec.rs`）。
- **日本語の仕様**：要件とシナリオの名前を日本語にした仕様も `openspec validate` を通る。ただし本文に `SHALL` か `MUST` の語が要るので、例では「（SHALL）」と書いた。
