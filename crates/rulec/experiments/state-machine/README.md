# 状態遷移の層（検討メモ）

2026-09-25 の検討。試作は rulec 0.19.1（作業ツリーのデバッグビルド）と Python 3.14 で走らせた。

**同じ日に実装した**（DESIGN §15.148）。構文は下の「設計案」のとおりで、`scenario` を足した。書き下ろしの規則は `tests/corpus/注文の状態.rule`、文法は docs/reference.md の §6.4。このメモの試作の規則と探索のスクリプトは、実装の前の記録として残す。

## 問い

状態遷移を扱う小さな言語を rulec と組み合わせると、何が書けるようになるか。証明できるままでいられるか。似たものは既にあるか。作るなら、どういう形になるか。

## 結論

- 組み合わせる価値はある。書けるようになるのは、注文・申請・稟議・会員ランク・契約のように段階を踏むもの。
- 別の言語を横に並べるより、rulec の表を「一歩ぶんの判断」に使い、遷移の層を宣言として足すほうがよい。単位も丸めも各言語への生成も `doc` も、そのまま使える。
- 決まる範囲に収めるための線は四つ。状態は有限の列挙にする。数を足し算で持ち越さない。並行を入れない。暦の計算を入れない。
- 業務の決定表、ライフサイクル、網羅的な検査、業務向けのコード生成。この四つを兼ねるものは見つからなかった。いちばん大きな空きは、表を改定したときに、処理中の案件を新しい版へ移して安全かどうかの証明。

## 試作

注文の状態遷移を、いまの rulec の表で書いた。現在の状態と届いた出来事を入力に、次の状態・返金額・受理を出力にする。取消の行には、わざと欠陥を一つ入れてある。取消のあとに入金通知が届くと、入金済に戻る。

### 規則（`注文の遷移.rule`）

```rule
rule 注文の遷移(order_step) v1
description "注文の状態と届いた出来事から、次の状態と返金額を決める。実験用のスケッチ"

enum 状態(state) = 受付(received) | 入金済(paid) | 出荷済(shipped) | 配達済(delivered) | 取消(cancelled)
enum 出来事(event) = 入金(pay) | 出荷(ship) | 配達(deliver) | 取消依頼(cancel)

inputs
  状態(state)   : 状態
  出来事(event) : 出来事
  支払額(paid)  : money[円, incl_tax]  range >=0円 <=100万円

outputs
  次の状態(next_state) : 状態
  返金額(refund) : money[円, incl_tax]  round down(1円)
  受理(accepted) : bool

table 遷移(step)
policy unique
| 状態   | 出来事               | -> 次の状態(next_state) : 状態 | 返金額(refund) : money[円, incl_tax] | 受理(accepted) : bool |
| 受付   | 入金                 | 入金済                         | 0円                                  | true                  |
| 受付   | 取消依頼             | 取消                           | 0円                                  | true                  |
| 受付   | 出荷, 配達           | 状態                           | 0円                                  | false                 |
| 入金済 | 出荷                 | 出荷済                         | 0円                                  | true                  |
| 入金済 | 取消依頼             | 取消                           | 支払額                               | true                  |
| 入金済 | 入金, 配達           | 状態                           | 0円                                  | false                 |
| 出荷済 | 配達                 | 配達済                         | 0円                                  | true                  |
| 出荷済 | 入金, 出荷, 取消依頼 | 状態                           | 0円                                  | false                 |
| 配達済 | -                    | 状態                           | 0円                                  | false                 |
| 取消   | 入金                 | 入金済                         | 0円                                  | true                  |  # わざと入れた欠陥
| 取消   | 出荷, 配達, 取消依頼 | 状態                           | 0円                                  | false                 |
```

出力のセルの `状態` は入力の名前で、「いまの状態に留まる」を表す。

### 一歩ぶんの検査

出荷済の行から `取消依頼` を外すと、E101 が抜けた組を出す。

```
error[E101]: 完全性の欠落: どの行にも当てはまらない入力があります
  --> gap.rule:17 表 遷移
   |
17 | table 遷移(step)
   |       ^^^^ 起こりうる入力を覆いきっていません
   |
 当てはまらない例: 状態 = 出荷済, 出来事 = 取消依頼
```

「出荷のあとに取消依頼が来たらどうするか」という問いになる。重なりの検査は、同じ状態に同じ出来事が来たときの行き先が一つに決まることを保証する。

欠陥の入った上の規則は、表としては完全で一意なので通る。

```console
$ rulec check 注文の遷移.rule
ok 注文の遷移.rule
```

### 探索のスクリプト（`explore.py`）

生成した Python の関数を使い、状態を幅優先でたどる。性質は、手順に沿って進む小さなオートマトン（`monitor`）で見張る。支払額はどの条件のセルにも出てこないので、値を一つ選べば全部を代表する。

```python
# A prototype of the missing check: walk every sequence of events from the
# initial state, using the generated step function, and look for the shortest
# trace that breaks a property stated across steps.
import sys
from collections import deque
sys.path.insert(0, "gen/python")
from order_step import order_step, State, Event, YenInclTax

paid = YenInclTax(3000)   # 支払額 is in no condition cell, so one value stands for all
name = {State.RECEIVED: "受付", State.PAID: "入金済", State.SHIPPED: "出荷済",
        State.DELIVERED: "配達済", State.CANCELLED: "取消",
        Event.PAY: "入金", Event.SHIP: "出荷", Event.DELIVER: "配達", Event.CANCEL: "取消依頼"}

def search(bad, monitor, m0):
    """BFS over (state, monitor); the monitor is a tiny automaton over steps."""
    start = (State.RECEIVED, m0)
    seen, q = {start: None}, deque([start])
    while q:
        node = q.popleft()
        s, m = node
        for e in Event:
            out = order_step(s, e, paid)
            if not out.accepted:
                continue
            m2 = monitor(m, s, e, out)
            nxt = (out.next_state, m2)
            if nxt in seen:
                continue
            seen[nxt] = (node, e, out)
            if bad(nxt):
                trace, cur = [], nxt
                while seen[cur] is not None:
                    prev, ev, o = seen[cur]
                    trace.append(f"--{name[ev]}{'（返金 ' + str(int(o.refund)) + '円）' if o.refund else ''}--> {name[cur[0]]}")
                    cur = prev
                return "受付 " + " ".join(reversed(trace))
            q.append(nxt)
    return None

states_reached = set()
q = deque([State.RECEIVED]); states_reached.add(State.RECEIVED)
while q:
    s = q.popleft()
    for e in Event:
        o = order_step(s, e, paid)
        if o.accepted and o.next_state not in states_reached:
            states_reached.add(o.next_state); q.append(o.next_state)
print("着ける状態:", " ".join(name[s] for s in State if s in states_reached))

# 1. once cancelled, never shipped
w = search(lambda n: n[1] and n[0] == State.SHIPPED,
           lambda m, s, e, o: m or o.next_state == State.CANCELLED, False)
print("取消のあとに出荷済へ着く手順:", w or "無し")

# 2. at most one refund per order
w = search(lambda n: n[1] >= 2,
           lambda m, s, e, o: min(2, m + (1 if o.refund > 0 else 0)), 0)
print("返金が二度起きる手順:", w or "無し")
```

### 走らせ方

上の二つを同じディレクトリに置いて、次のように走らせる。

```console
$ rulec gen 注文の遷移.rule --out gen
$ python3 explore.py
着ける状態: 受付 入金済 出荷済 配達済 取消
取消のあとに出荷済へ着く手順: 受付 --取消依頼--> 取消 --入金--> 入金済 --出荷--> 出荷済
返金が二度起きる手順: 受付 --入金--> 入金済 --取消依頼（返金 3000円）--> 取消 --入金--> 入金済 --取消依頼（返金 3000円）--> 取消
```

取消と入金通知が行き違い、取り消した注文が出荷される。一件ずつの判定をいくら検査しても見つからない種類の欠陥である。

## なぜ証明できるままでいられるか

rulec の検査が必ず終わるのは、セルが自分の列を定数と比べるだけで、入力空間が有限個の区画に切れるからである。状態遷移でも、次の二つが成り立てば「状態 × 区画」は有限になる。

- 持ち越す状態が有限の列挙である
- 数値は呼び出しのたびに渡され、表の定数で区画に切られる

このとき、着けない状態や行き詰まる状態があるか、「一度取消になったら出荷済には着かない」が成り立つかは、どれもグラフの探索で厳密に決まる。反例は最短の手順として出せる。辺は表の行から読め、行に届く入力があるかは E102 がすでに判定している。

これは fold（DESIGN §15.56「判定の列に対する小さなオートマトン」）の延長にあたる。fold は形の決まったオートマトンで、こちらは状態を利用者が宣言するオートマトンである。

## 線を引く場所

入れると決まらなくなるもの：

- **足し算で数を持ち越すこと**（返金の累計、残高など）。カウンタ機械になり、一般には決まらない。持ち越した値を定数や他の値と比べるだけなら検査が終わる、という結果はある（Felli・Montali・Winkler、CAiSE 2022。データつきペトリネットで、ガードが変数どうし・変数と定数の比較だけの場合）。累計はいまと同じく、呼び出し側が持って入力で渡す。
- **複数のステートマシンの並行**。メッセージの順序や競合は TLA+ や P の領分で、状態の数も爆発する。
- **暦の計算**。「14 日以内」なら、経過日数を呼び出し側が渡し、表は定数と比べるだけにする。時計を持たせる形（タイムオートマトン。時計を定数とだけ比べるなら決まる）は、最初には入れない。

入力の種類も宣言で分ける。呼び出しのたびに変わる入力と、案件のあいだ変わらない入力（支払額など）である。後者を毎回自由に選ばせると、実際には起きない手順まで反例に出る。rulec は「反例は実際に送れる入力」と約束しているので、この区別が要る。後者を案件の始まりに一度だけ選ぶ値として扱えば、区画は有限のままである。

## 既存のもの

2026-09-25 に、論文・公式の文書・リポジトリにあたって調べた。

| | 何をするか | rulec の層との違い |
|---|---|---|
| SCR（米海軍研究所、1980 年前後から） | 条件表・イベント表・モード遷移表。検査器がどの表でも行の重なりを見る（Heitmeyer ら、TOSEM 1996）。SPIN、TAME/PVS、Salsa、C コードの生成まであった | 抜けを見るのは条件表だけ。イベント表とモード遷移表は部分的でよく、書いていない遷移は「留まる」と読む。公開の入手先が見当たらない |
| RSML（Leveson ら、TCAS II） | AND/OR 表を遷移の条件にした階層つきステートマシン。完全性と決定性を検査する（Heimdahl・Leveson、TSE 1996）。派生の RSML-e は Nimbus で動き、NuSMV と PVS へ変換できる | 航空の安全系向け。後継の SpecTRM-RL は Safeware Engineering の商用の道具で使う |
| Stateflow + Simulink Design Verifier | 真理値表の抜けと過剰を既定で誤りにする。SLDV（商用）は死んだ論理を見つけ、性質を証明し、テストを作る | 遷移表の検査は構造だけ。条件は順序つきで、重なりは優先順位で片づき、報告されない |
| Kind 2（Lustre） | 契約のモードが、仮定の許す状況を全部覆うかを検査し、各モードに届くかも見る。Rust への変換もある | 組み込み・同期言語向け |
| Rebel（ING と CWI、2016） | 銀行の商品（口座、振込など）のライフサイクルを宣言的に書き、SMT でシミュレーションと検査をし、コードを生成して動かす（Scala/Akka） | 業務の例ではいちばん近い。表ではなく事前・事後条件で書く。後継の Rebel2 は有界の検査 |
| Marlowe（Cardano） | 金融契約の小さな言語。期限が来れば必ず閉じる。お金の総額が保たれること、取引の数に上限があることを Isabelle で証明し、実行時の警告が起きないことを SMT で調べる | わざと小さくして証明する考え方は同じ。対象はブロックチェーン上の契約 |
| Symboleo | 契約の義務と権限を、ライフサイクルの状態図に対する公理で定める。SymboleoPC が、利用者の書いた LTL/CTL を nuXmv で検査する | 抜けを出す検査は組み込まれていない |
| L4（Legalese、SMU の CCLAW） | いまの jl4 は PARTY、MUST/MAY/SHANT、WITHIN、HENCE/LEST を書け、#TRACE のシナリオを走らせて確かめる。状態グラフの表示もある | UPPAAL の件は手で組んだ事例研究（WAICOM 2022）で、変換器ではない。UPPAAL・SPIN・NuSMV への変換は計画段階の仕様（2026-01） |
| eFLINT | 事実・行為・義務を遷移系として書き、与えた手順の適合を見る（GPCE 2020）。nuXmv への実験的な変換（2022）と、利用者が区切った範囲でシナリオを探す Clingo の解釈器（GPCE 2025）がある | 網羅は、区切った範囲の中の探索 |
| ワークフローネット、YAWL | 健全性（必ず終われる、きれいに終わる、死んだ遷移が無い）が決まる。自由選択ネットなら多項式時間、一般には EXPSPACE 完全。道具は Woflan | コードは出さない。YAWL の取消（リセットネット）を入れると決まらない |
| BPMN + DMN、データつきペトリネット | DMN の判断のあとも処理が続けられ、後の作業がどれも実行できるかという健全性（Batoulis・Weske、2017）。ada がデータつきペトリネットの健全性を SMT で検査する | コードは出さない |
| TLA+ / Quint、P、Alloy 6 | 汎用の仕様言語とモデル検査。P は AWS で S3 の強い整合性への移行の検査に使われ、PObserve がログを見る。Alloy 6 は可変の状態と過去演算子つきの LTL を持つ。Quint は Informal Systems から独立した会社に移った | 分散システムの技術者向け。承認者は読めず、rulec の単位や丸めとつながらない |
| Ivy、Veil（Lean 4） | 遷移系の検証。Ivy は検証条件が決まる断片（EPR）に収まるよう言語を絞った。Veil は決まる範囲を SMT で自動に解き、外れたら対話的な証明に落とす | 決まる断片に言語を絞る考え方は同じ。Veil は、rulec の `proofs/` が Lean 4 なので参考になる |
| XState、Temporal、Camunda | 状態遷移やワークフローを実行する。XState は `xstate/graph` で手順を生成し、モデルベースのテストを作れる | 性質は証明しない。rulec の関数を各段で呼ぶホストの側 |

四つを兼ねるものは無かった。部分ごとに近いのは次のとおり。

- 表・モード・網羅的な検査・コード生成を持つが、組み込み向け：SCR（C コード）、Stateflow + SLDV、Kind 2
- 業務のライフサイクルに検証をかける：Rebel、Rebel2（有界）、Symboleo、Stipula（KeY、2025）、VERIFAS（VLDB 2017）
- BPMN + DMN：ada は健全性を検査するが、コードは出さない。BDTransTest（2025-12）は Java とテストを出すが、証明はしない

形がいちばん近い SCR と RSML は、1980〜90 年代に同じ組み合わせを軍事・航空でやっている。表とステートマシンを組み合わせて証明するという考え自体には実績がある。

### 処理中の案件の移行

表を改定したとき、処理中の案件を新しい版へ移して安全か。理論は 2000 年代に整理されている。van der Aalst（2001）は、案件を新しい定義へ移したときに作業の重複・飛ばし・デッドロック・ライブロックが起きることを、dynamic change bug と呼んだ。Rinderle・Reichert・Dadam（2004）は、移行の正しさの基準を整理した。

いまの実行エンジンは構造しか見ない。

- Temporal：再生が合わなければ、非決定性のエラーで止まる。文書は、実行時の検査は徹底したものではないとして、再生テストを勧めている
- Camunda 8：移行の検査は構造だけ。文書に「問題のある状況は検出しないので、適切な移行を選ぶのは利用者」とある

2022〜2026 年のもので、移行が安全だと証明するものは見つからなかった。状態が有限で、辺が検査済みの表の行であれば、これは決まる問題である。

## 設計案

新しい言語ではなく、rulec の宣言を一つ足す。構文は仮。

```
machine 注文(order) over 遷移
  carry   状態 -> 次の状態
  initial 受付
  final   配達済, 取消
  never   出荷済 after 取消
  once    返金額 >0円
```

- 新しく加わる意味は `carry`（この出力が、次の呼び出しでこの入力になる）だけ。
- 性質は、承認者が読める少数の形に絞る。一般の時相論理は入れない。`constraint` で一般の論理式を捨てたのと同じ理由である。
- `final` を宣言すれば、試作の欠陥は「終わりの状態である取消から出る行がある」として、一歩ぶんの検査でも見つかる。
- 終わりの状態から出る行が無いことと、どの状態からも終わりに着けること（ワークフローネットの健全性の「必ず終われる」にあたる）は、宣言しなくても検査する。
- 生成コードは、状態を受け取って次の状態を返す純関数のまま。状態を保存するのは今までどおりホストで、DESIGN §5.4 の「状態はホストが持ち、P3 のままである」とも合う。

いまの検査との対応：

| いまの rulec | 状態遷移での意味 |
|---|---|
| E101 抜け | 処理が決まっていない（状態, 出来事）の組。SCR と違い、留まる場合も書かせる |
| E105 重なり | 行き先が二つある遷移 |
| E102 届かない行 | 着けない状態、使われない遷移 |
| witness の入力 | 最短の手順 |
| examples | 手順の例 |
| 畳み込みの遷移カバー | 遷移と、続けて起きる遷移の対のカバー |
| `diff` | 旧版と新版で結果が変わる手順と、その最短の一本 |
| `replay` | 過去のイベントログを新しい版に通し、途中で断られる案件を数える |

価値がいちばん大きいのは最後の二つで、上の「処理中の案件の移行」に答える。

## 気をつけること

- **記録済みの決定を開き直す。** DESIGN §0.1 は「ワークフローとステートマシン」を捨てたものに挙げ、サイトの「向いていないもの」にもワークフローがある。§0.1 が捨てたのは「規則が状態を持つこと」で、ここで足すのは「ホストが持つ状態について、呼び出しの並びを証明すること」である。この違いを、新しい決定として DESIGN.md に書くことになる。
- **費用のかかる場所。** 検査の本体は、状態のグラフと、性質を見張るオートマトンを組み合わせた探索なので小さい。重いのは、機能を gen・test・api・doc の全部に載せるための周りである。手順のベクタ、各言語のランナーが step を繰り返し呼ぶ形、網羅の基準、`doc` の状態遷移図、`diff` と `replay`、証明書と `proofs/` への到達グラフの検査が要る。

## 決まっていないこと

- `machine` を `.rule` の中に置くか、別のファイルにするか
- 性質の形を、`never … after`、`once`、`final` のほかに足すか（`reachable` など）
- 案件のあいだ変わらない入力を、どう宣言するか
- 時計（経過時間を定数と比べる）をいつ入れるか
- SQL に、許されない遷移を断るトリガーを出すか
- 到達グラフの検査を、証明書と `proofs/` でどう再検査するか

## 出典

- SCR：Heitmeyer, J.UCS 13(5), 2007. <https://www.jucs.org/jucs_13_5/formal_methods_for_specifying/jucs_13_5_0607_0618_heitmeyer.pdf>
- RSML：Heimdahl, Leveson, "Completeness and Consistency in Hierarchical State-Based Requirements", IEEE TSE 22(6), 1996. <https://ieeexplore.ieee.org/document/508311>
- Stateflow：<https://www.mathworks.com/help/stateflow/ug/correcting-overspecified-and-underspecified-truth-tables.html>
- Kind 2：<https://kind.cs.uiowa.edu/kind2_user_docs/v2.1.1/9_other/2_contract_semantics.html>
- Rebel：Stoel, van der Storm, Vinju, Bosman, "Solving the Bank with Rebel", ITSLE 2016. <https://dl.acm.org/doi/10.1145/2998407.2998413>
- Marlowe：WTSC 2020. <https://fc20.ifca.ai/wtsc/WTSC2020/WTSC20_paper_18.pdf>
- Symboleo：<https://github.com/Smart-Contract-Modelling-uOttawa/Symboleo-Model-Checker>
- L4：<https://legalese.com/l4/concepts/legal-modeling/regulative-rules>
- eFLINT：GPCE 2025. <https://ltvanbinsbergen.nl/files/papers/gpce2025-eflint.pdf>
- ワークフローネットの健全性の計算量：Blondin ら, LICS 2022. <https://arxiv.org/abs/2201.05588>
- データつきペトリネット：Felli, Montali, Winkler, CAiSE 2022. <https://arxiv.org/abs/2203.14809>
- BPMN + DMN の健全性：Batoulis, Weske, 2017. <https://link.springer.com/chapter/10.1007/978-3-319-69904-2_31>
- DMN の表の解析：Calvanese ら, BPM 2016. <https://arxiv.org/abs/1603.07466>
- Quint：<https://github.com/quint-co/quint>
- P：Brooker, Desai, "Systems Correctness Practices at AWS", CACM 68(6), 2025. <https://dl.acm.org/doi/10.1145/3729175>
- Alloy 6：<https://alloytools.org/alloy6.html>
- Ivy：<https://github.com/kenmcmil/ivy>
- Veil：<https://github.com/verse-lab/veil>
- XState：<https://stately.ai/docs/graph>
- van der Aalst, "Exterminating the Dynamic Change Bug", Information Systems Frontiers 3(3), 2001. <https://link.springer.com/article/10.1023/A:1011409408711>
- Rinderle, Reichert, Dadam, Data & Knowledge Engineering, 2004. <https://www.sciencedirect.com/science/article/abs/pii/S0169023X04000035>
- Temporal：<https://docs.temporal.io/develop/go/versioning>
- Camunda 8：<https://docs.camunda.io/docs/components/concepts/process-instance-migration/>
