# chobo 実装計画

DESIGN.md が仕様で、この計画はそれを作る順序と、段階ごとの終わりの条件を決める。段階は三つある。

- **B**：言語の芯（字句・構文・型）、検査、参照インタプリタ、シナリオの生成と同時の順序の数え上げ、診断（英語と日本語）、CLI の `check`・`run`・`scenarios`・`api`・`explain`
- **C**：PostgreSQL と TigerBeetle への出力、三つの言語のクライアント、突き合わせのテスト（同時のシナリオを含む）
- **D**：`doc`、例、README.md と README.ja.md、エージェント向けのスキル（`skills/chobo`）

実装して DESIGN.md の決定が成り立たないと分かったら、黙って変えずに、DESIGN.md を理由と捨てた形ごと直す。0 章の約束（形式、ID、チェーンの並び、名前）を変えるときも同じで、変えたら、それを使う段階のテストも直す。

## 0. どの段階にも共通すること

### 0.1 決まり

- Rust は edition 2024、手元の stable 1.94.1 で通ること。依存は `serde_json`（`preserve_order`）だけ。SHA-256 も自分で書く。
- git のコミットと push はしない。`~/chobo` の外（`~/rulec`、`~/dandori` ほか）には書かず、そこでビルドもしない。読むのは自由で、rulec と dandori の `src` と `tests` は書き方の手本になる（dandori の `src/diag.rs`、`src/check.rs`、`src/scenarios.rs`、`tests/examples.rs`、`tools/temporal-go`）。
- 一時ファイルはテストの一時ディレクトリ（`std::env::temp_dir()` の下の `chobo-test-<pid>-…`）に置き、終わったら消す。テストのプロセスは最初に、終わったプロセスの残したディレクトリを消す（dandori で、誰も消さずに 6.8 GB たまったことがある）。
- サーバー（PostgreSQL、TigerBeetle）を立てたまま終わらない。テストが失敗しても止めて消す（`Drop` で）。TigerBeetle のデータファイルは一つ 1.1 GB あり、レプリカは初期化で 1.5 GB のメモリを取るので、一つのテストのプロセスで立てるのは一つだけにする。
- Go のコマンドには `-trimpath` を付ける（付けないとビルドキャッシュが一回に 1 GB 近く増える）。
- 診断は英語を既定にし、`--lang ja` か `CHOBO_LANG=ja` で日本語。日本語は普通の言い方で書く（英語の概念語を漢字に直訳した造語をしない。カタカナ英語が普通の語はカタカナで）。DESIGN.md の語（勘定、振替、移動、仮押さえ、確定、取消、有効期限、境界、拒否、キー、テナント）にそろえる。
- 外の道具（PostgreSQL、TigerBeetle、node、python、go、それぞれの依存）が無ければ、`SKIP: <理由>` の行を出して通す。終わりを報告する前に、`cargo test -- --nocapture` で SKIP の行を数える。
- golden の書き直しは `CHOBO_BLESS=1`。書き直したら差分を読む。
- 文書（DESIGN.md、README、docs）に載せる出力と数は、実際に走らせたものを貼る。DESIGN.md のスケッチ（1.1 の帳簿、3 章の診断と報告、4.3 のクライアントの形）は、その段階で実物に差し替える。
- 例には公開されているものか、例のために書いたものを使う。手元のマシン名、ユーザー名、絶対パスを、成果物（文書、golden、生成物）に入れない。

### 0.2 置き場所

```
Cargo.toml            edition 2024、serde_json だけ
src/
  main.rs             コマンドの表、引数の解析、--help（B）
  lib.rs
  syntax.rs           字句、字下げ、KEYWORDS の表（B）
  parse.rs            構文木（位置つき）（B）
  model.rs            名前と単位を解決した帳簿（B）
  interp.rs           参照インタプリタ（B）
  scenario.rs         シナリオと結果の JSON、together の数え上げ（B）
  scenarios.rs        シナリオの生成（B）
  check.rs            検査と、振替の種類ごとの報告（B）
  witness.rs          操作の列を探して参照インタプリタで確かめる（B）
  diag.rs             診断（テキストの英日、JSON）、tr!（B）
  codes.rs            診断の台帳（explain の文、最小の再現）（B）
  ids.rs              SHA-256、ID、TigerBeetle のチェーンの並び（B）
  api.rs              chobo api（B）
  diffbase.rs         --diff-base（B）
  render.rs           chobo run --show（C）
  postgres.rs         SQL、クライアントが送る SQL の文、E061（C）
  target.rs           七つのターゲットと、それぞれが書くファイル（C）
  client/             mod.rs（E060、帳簿ごとの部分の材料）、typescript.rs、python.rs、go.rs（どれも PostgreSQL と TigerBeetle の二つ）（C）
  client/runtime/     どの帳簿でも同じ部分：tigerbeetle.ts・.py・.go、postgres.ts・.py・.go（C）
  doc.rs              chobo doc の中身（文、表、シナリオのステップごとの残高）と Markdown（D）
  draw.rs             HTML のページ、流れの図とライフサイクルの図の SVG、シナリオを進めるスクリプト（D）
tests/
  common/mod.rs       golden、一時ディレクトリ（終わったプロセスの残りとサーバーも消す）、chobo のコマンド、例の帳簿の一覧（B、C、D）
  common/servers.rs   使い捨ての PostgreSQL のクラスタと TigerBeetle のレプリカ（C）
  common/runners.rs   テストのコピー、クライアントのビルド、ランナー、psql で SQL を流すもの、突き合わせ（C）
  fixtures/           診断の帳簿と golden（<名前>.book、<名前>.en.txt、<名前>.ja.txt。--diff-base の比べる側は <名前>.before.book、build の診断はビルドするターゲットを <名前>.target に）。名前は日本語で書く（README が見せる split.book だけ英語）（B、C、D）
  books/              テストの帳簿と golden（<名前>.book、.runs.json、.chains.json、.api.json）、手で書いたシナリオ（<名前>.more.json）とその結果（<名前>.more.runs.json）。名前は日本語で書く（B、C、D）
  doc/                chobo doc の golden：テストの帳簿の <名前>.en.md、<名前>.ja.md、<名前>.html、例の <名前>.html、警告のある帳簿の 順序.en.md と 順序.ja.md（D）
  semantics.rs、check.rs、scenarios.rs、ids.rs、api.rs、cli.rs、diffbase.rs（B）
  backends.rs、postgres.rs、tigerbeetle.rs、generated.rs（C）
  doc.rs、docs.rs、skill.rs（D）
tools/
  tigerbeetle/fetch.sh  リリースのバイナリを取ってきて SHA-256 を確かめる（C）
  runner/               package.json と package-lock.json、runner.ts、requirements.in と requirements.txt（ハッシュつき）、runner.py（C）
  runner/go/            go.mod、go.sum、harness/（Go のランナーの共通部分。テストが生成物と main.go を足して一つのバイナリにする）（C）
  mermaid/              package.json と package-lock.json（Mermaid 11 と 12。tests/doc.rs が図を描くのに使う。入れ方は npm ci --prefix tools/mermaid）（D）
examples/<名前>/      四つの例：<名前>.book（英語）と <名前>.ja.book（日本語）、chobo doc のページ doc.md と doc.ja.md、返金の例は手で書いたシナリオ <名前>.more.json とその結果（D）
docs/                 reference.md、formats.md、targets.md、codes.md、codes.ja.md（D）
skills/               chobo/（SKILL.md と docs/ のページのコピー）、sync.sh（コピーを作る）、README.md（D）
README.md、README.ja.md（D）。LICENSE-MIT と LICENSE-APACHE（2026-10-03 に作者が MIT OR Apache-2.0 に決めた）
```

### 0.3 段階をまたぐ約束

#### シナリオの JSON

```json
{
  "name": "bound: 引当.hold takes 在庫(sku) to exactly 0, its `at least 0`",
  "book": "在庫",
  "steps": [
    {"op": "do", "kind": "入荷", "args": {"納品書": "納品書-1", "sku": "sku-2", "数": 3}},
    {"op": "hold", "kind": "引当", "args": {"注文": "注文-3", "sku": "sku-2", "数": 3}},
    {"op": "post", "kind": "引当", "args": {"注文": "注文-3", "sku": "sku-2"}, "amounts": {"数": 2}},
    {"op": "void", "kind": "引当", "args": {"注文": "注文-4", "sku": "sku-2"}},
    {"op": "pass", "duration": "31 minutes"},
    {"op": "together", "callers": [[{"op": "hold", "kind": "引当", "args": {}}], [{"op": "hold", "kind": "引当", "args": {}}]]}
  ]
}
```

- `post` と `void` の `args` はキーの引数だけ。`post` の `amounts` は額の引数を全部か、書かない（全額）か。
- 額は JSON の整数。生成するシナリオでは 2⁵³ − 1 以下にする（JavaScript の数で読めるように）。
- `together` の `callers` は、呼び出し元ごとの操作の並び。`together` の中に `pass` と `together` は書けない。一つの `together` の操作は合わせて 8 まで（超えれば読むときにエラーにする）。
- `pass` の `duration` は `<整数> <second(s)|minute(s)|hour(s)|day(s)>`。生成するシナリオは、割り切れる一番大きい単位で書く（`31 minutes`）。
- 生成するシナリオの `name` は、種類を表す語で始まる：`bound:`（境界の手前・ちょうど・超える）、`key:`（同じキーの二度目）、`hold:`（仮押さえの終わり方）、`pass:`（期限切れ）、`moves:`（移動の途中での拒否）、`together:`（同時）、`same_account:`。手で書いたシナリオ（`<帳簿>.more.json`）は `more:` で始める。名前に入る数は、単位の `scale` の桁で書く（段階 D で決めた。DESIGN 7）。文字列の値は `<引数の名前>-<番号>` で、番号はシナリオの中で最初に出てくる順に振る。`chobo doc --lang ja` のための日本語の題は、`scenarios::generate_titled` が名前と一緒に作る（JSON には入れない）。

#### 結果の JSON（`chobo run --format json` と、ランナーの出力）

```json
{
  "steps": [
    {"op": "do", "kind": "入荷", "result": "done"},
    {"op": "hold", "kind": "引当", "result": "refused", "reason": "在庫切れ"},
    {"op": "pass"},
    {"op": "together", "callers": [[{"op": "hold", "kind": "引当", "result": "done"}],
                                   [{"op": "hold", "kind": "引当", "result": "refused", "reason": "在庫切れ"}]]}
  ],
  "accounts": [
    {"account": "在庫", "args": ["sku-2"], "posted": 3, "held_in": 0, "held_out": 0},
    {"account": "仕入先", "args": [], "posted": -3, "held_in": 0, "held_out": 0}
  ],
  "holds": [{"kind": "引当", "key": ["注文-3", "sku-2"], "state": "posted"}]
}
```

- `steps` はシナリオの一つのステップに一つ。`together` は `callers` の中に、呼び出し元の順、その中は書いた順に結果を並べる（処理された順ではない）。
- `accounts` は、シナリオの `do` と `hold` が指すすべての勘定（外の勘定も。拒否された操作のものも）。勘定の種類の宣言の順、その中は引数の値の順（バイトの順）に並べる。データベースに行が無ければ 0 として読む。シナリオだけから求められるので、ランナーはクライアントの `balance` で読めばよい。
- `holds` は作った仮押さえを、振替の種類の宣言の順、その中はキーの値の順に。`state` は `held`・`posted`・`voided`・`expired`。
- `together` のあるシナリオでは、参照インタプリタは `{"outcomes": [<上の形>, …]}`（重ねず、文字列の順に並べる）を出し、ランナーは一つの結果を出す。ランナーの結果が `outcomes` のどれかと等しければ合う。

#### ID（DESIGN 4.3）

`enc(s)` は UTF-8 のバイト長をビッグエンディアンの 32 ビットで前に付けたもの。`H(…)` は `SHA-256(enc("chobo/1") ‖ enc(p1) ‖ enc(p2) ‖ …)`。ID は `H` の先頭 16 バイトをビッグエンディアンで読み、0 なら 1、2¹²⁸ − 1 なら 2¹²⁸ − 2 にする。ledger は先頭 4 バイト（0 なら 1）、code は先頭 2 バイト（0 なら 1）。16 進は小文字で 32 桁。段階 A に Python の hashlib で求めた値で、Rust と三つの言語と PostgreSQL の `chobo_id` は、どれもこれと一致しなければならない。

| 部品 | 値 |
|---|---|
| `account, 在庫, "", 在庫, A-1` | `396213c27529b522708ee6da1d0d03f7` |
| `account, inventory, "", stock, A-1` | `f0857e5a2049d01a7f4989e04ee4e37f` |
| `account, inventory, t-1, stock, A-1` | `9a98b9b990f5c3ea4640cd5755b4ce17` |
| `account, inventory, "", supplier` | `cafb18eaf6485ea880c3ef6350f4cdeb` |
| `transfer, 在庫, "", 引当, hold, o-1, A-1, 0` | `594bbb54c47e8b03c55fac1644fbcffa` |
| `transfer, 在庫, "", 引当, post, o-1, A-1, 0` | `0a75517949dcb9dec36a9a9d9b04a50c` |
| `sink, 在庫, "", 個` | `e132a4987f7c612337996fa93bc06f65` |
| `room, f0857e5a2049d01a7f4989e04ee4e37f` | `2bd360017fee460bbca3ca67451e63dc` |
| `opening, 2bd360017fee460bbca3ca67451e63dc` | `c5af9cb7947230b39cfaa34fff20953b` |
| `account, b, "", a:b, c` | `464fb5f164fc1b9fd9fc7ea1157c5aaf` |
| `account, b, "", a, b:c` | `4f01a8c48b5a8baed7db4f9df67b300b` |
| ledger `個, 0` | 1653847866 |
| ledger `USD, 2` | 1838891465 |
| code `在庫, transfer, 引当` | 47038 |
| code `在庫, account, 在庫` | 48630 |

ほかの部品：`floor` は `room` と同じ形。`kind` は（帳簿、テナント、勘定の種類）。`content` は（帳簿、テナント、振替の種類、操作、定義のハッシュ、中身の JSON）。

- **定義のハッシュ**：`H("definition", <振替の種類の文>, <移動が触る勘定の種類の文>…)` の先頭 16 バイトの 16 進（小文字 32 桁）。勘定の種類は、移動を書いた順に、元、先の順で最初に出てきた順に並べる。`description` と拒否の理由の名前は入れない（保存するものを変えないので）。
  - 振替の種類の文は、次の行を `\n` でつないだもの：`transfer <名前>(<引数>: <string か単位の名前>, …)`、`key <引数>, …`（キーの順）、`pending expires after <秒> seconds` か `pending never expires`（仮押さえのときだけ）、移動ごとに `move <額> from <勘定> to <勘定>`。額は引数の名前か、いちばん小さい単位の整数。勘定は `名前` か `名前(<引数の名前か "文字列">, …)`（文字列の中の `"` と `\` は `\` を前に付ける）。
  - 勘定の種類の文は `account <名前>(<引数>: string, …) : <単位> scale <桁数>` に、外の勘定なら ` outside`、下限があれば ` at least <整数>`、上限があれば ` at most <整数>` を続けたもの（引数の無い勘定は括弧を書かない）。
  - 書いているのは `src/ids.rs` の `transfer_text` と `account_text`。`chobo api` の `transfers[].definition` が、このハッシュ。
- **中身の JSON**：`do` と `hold` は、全部の引数を宣言の順に並べたオブジェクト（`{"注文":"o-1","sku":"A-1","数":3}`）。`post` は移動ごとの確定する額の配列（全額なら押さえた額。`[3]`）、`void` は `null`。空白を入れない。文字列は JavaScript の `JSON.stringify` と同じに書く（`"` と `\` と `\b` `\f` `\n` `\r` `\t` をエスケープし、ほかの U+0020 より前の文字は小文字の `\u00xx`、それ以外はそのまま）。Go の `encoding/json` は既定で `<` `>` `&` と U+2028・U+2029 もエスケープするので、クライアントでは使わない。
- 勘定の `user_data_128` は `kind` の ID。

#### TigerBeetle のチェーンの並び

`do` と `hold` は、移動ごとに次の順に振替を並べる（書いた移動の順に、移動ごとにこの並びを続ける）。A は元の勘定、B は先の勘定、a は移動の額。

| 役目（`user_data_32`） | 借方（debit） | 貸方（credit） | 額 | 付けるとき | 仮押さえか |
|---|---|---|---|---|---|
| main（1） | A | B | a | いつも | `hold` なら |
| probe（2） | A | 受け皿 | A の下限 L | A の L > 0 | いつも（timeout 0） |
| probe_void（3） | （probe を取り消す） | | 0 | A の L > 0 | — |
| floor_out（4） | A の余裕 | 受け皿 | a | A の L < 0 | `hold` なら |
| room_back（5） | 受け皿 | A の空き | a | A に上限 | `hold` なら |
| room_in（6） | B の空き | 受け皿 | a | B に上限 | `hold` なら |
| floor_back（7） | 受け皿 | B の余裕 | a | B の L < 0 | `hold` なら |

- 最後の振替のほかは `linked`。ledger は単位の ledger、code は振替の種類の code、`user_data_128` は `content` の ID、`user_data_64` は 0。`hold` の振替（probe と probe_void を除く）の timeout は有効期限の秒（`never expires` なら 0）。
- ID は `transfer, 帳簿, テナント, 振替の種類, 操作, キーの値…, チェーンの中の位置（0 から、10 進）`。
- `post` と `void` のチェーンは、`hold` のチェーンから probe と probe_void を除いた振替を同じ順に並べ、それぞれの `pending_id` を対応する `hold` の振替の ID にする。debit・credit・ledger・code は 0（仮押さえから引き継ぐ）。`post` の額は、全額なら `amount_max`（2¹²⁸ − 1）、一部なら移動ごとに求め直した額（余裕と空きの振替も、その移動と同じ額）。`void` の額は 0。
- probe は ledger と code を振替の種類のものにし、`pending`、timeout 0。probe_void は `void_pending_transfer`、`pending_id` は直前の probe の ID、debit・credit・ledger・code・額は 0。
- 勘定のフラグ：下限が 0 以上の中の勘定と、余裕と空きの勘定に `debits_must_not_exceed_credits`。外の勘定、下限が 0 より小さい中の勘定、受け皿には付けない。勘定の `user_data_32` は 0（勘定）、1（余裕）、2（空き）、3（受け皿）。
- 勘定の code：勘定と、その余裕と空きは `code, 帳簿, account, 勘定の種類`。受け皿は `code, 帳簿, sink, 単位`。勘定の `user_data_128`：勘定と余裕と空きは、その勘定の種類の `kind` の ID。受け皿は 0。ledger はどれも単位の ledger。
- 作る勘定の並び（`chobo run --show tigerbeetle` とクライアントが送る順）：移動ごとに、A、B、（余裕・空き・probe のどれかがあれば）受け皿、そのあと floor_out・room_back・room_in・floor_back の順に要る余裕と空き。前に出てきた勘定は飛ばす。
- 開始の振替：余裕と空きの勘定ごとに一つ、勘定の並びと同じ順。ID は `opening, <余裕か空きの ID>`、借方は受け皿、貸方は余裕か空き、額は −L か U、ledger は単位の ledger、code はその勘定の種類の code、`user_data_32` は 8、`user_data_128` は 0、フラグなし。
- クライアントが何も送らないとき：`do` と `hold` で元と先が同じ勘定になる移動があるとき（`same_account`）と、`post` と `void` で仮押さえが無いとき（`no_such_hold`）。
- 段階 B の `src/ids.rs`（`chain`、`scenario_chains`）がこのとおりに組み、`tests/books/<帳簿>.chains.json` に、全シナリオの全操作の分をテナント `""` で固定してある。

#### PostgreSQL の名前

- スキーマ：帳簿の名前（引用符で囲む）。テーブル：`accounts`・`holds`・`keys`・`entries`。型：`"<帳簿>".result (result text, reason text)` と `"<帳簿>".balance (posted bigint, held_in bigint, held_out bigint)`。`keys` は中身（`jsonb`）のほかに、定義のハッシュ（`definition text`）を持つ。
- 関数：`"<振替の種類>_do"`・`_hold`・`_post`・`_void`・`_status`、`"balance_<勘定>"`、`expire(p_max integer default 1000)`、`chobo_id(parts text[]) returns uuid`（ID を SQL の中で求める。テストが 0.3 の表と比べる）。引数は（テナント text、帳簿の引数を宣言の順に。`string` は text、額は bigint）で、名前の前に `p_` を付ける（`p_tenant`、`"p_注文"`。帳簿の引数に `tenant` があれば、テナントの方を `p_tenant_` にする）。`_post`・`_void`・`_status` はキーの引数を宣言の順に取り、`_post` はそのあとに額の引数を `default null` で持つ（全部 null なら全額）。
- クライアントが送る SQL：操作は `select * from "<帳簿>"."<関数>"($1, …)`（引数は全部渡し、額の無い `_post` には null）、状態は `select "<帳簿>"."<種類>_status"($1, …) as state`、残高は `select * from "<帳簿>"."balance_<勘定>"($1, …)`、`expire` は `select "<帳簿>".expire() as expired`。Python は `$n` の代わりに `%s` を書く。`src/postgres.rs` の `call`、`status_sql`、`balance_sql`、`expire_sql` が、この文を書く。
- 一つの関数は一つのトランザクションの中で動く。分離レベルを自分で変えない。

#### クライアントの名前

- TypeScript：`postgres(db, { tenant })` と `tigerbeetle(client, { tenant })` が帳簿の値を返す。`<種類>.do(args)`、`<種類>.hold(args)`、`<種類>.post(key, amounts?)`、`<種類>.void(key)`、`<種類>.status(key)`、`balance.<勘定>(args)`、PostgreSQL では `expire()` も。結果は `{ result: "done" } | { result: "done_before" } | { result: "refused"; reason: Reason }`、状態は `"held" | "posted" | "voided" | "expired" | null`、残高は `{ posted, held_in, held_out }`。額は `bigint`。型は `<種類>Args`、`<種類>Key`、`<種類>Amounts`、`<勘定>Account`。ファイルは、Node が型を剥がすだけで走る書き方にする（`enum`、実行時のコードを含む `namespace`、コンストラクタの引数でのプロパティ宣言を使わない）。
- Python：`postgres(conn, tenant="")` と `tigerbeetle(client, tenant="")`。メソッドの名前は TypeScript と同じで、引数はキーワード引数（`post` の額は `None` が既定）。結果は `Result(result, reason)`、残高は `Balance(posted, held_in, held_out)`（どちらも `dataclass`）、状態は `str` か `None`。額は `int`。キーワードと `self` などの名前は後ろに `_` を付ける。
- Go：パッケージの名前は、帳簿の名前が ASCII の識別子ならそれ（小文字にする）、でなければファイル名（`inventory.ja.book` なら `inventory_ja`）、それも使えなければ `book`。`Postgres(q Querier, tenant string) *Book` と `TigerBeetle(c TigerBeetleClient, tenant string) *Book`（どちらもインターフェースを受け取り、テストが包めるようにする）。`b.X引当.Hold(ctx, X引当Args{…})`、`Post(ctx, X引当Key{…}, *X引当Amounts)`、`Void`、`Status`（`HoldState`、無ければ `""`）、`b.Balance.X在庫(ctx, X在庫Account{…})`、PostgreSQL では `b.Expire(ctx)`。外から見える名前で日本語の名前には前に `X` を付ける（`X引当`）。結果は `Result{Outcome, Reason string}`、失敗は `error`。額は `int64`。
- 三つの言語とも、振替の種類の名前は `balance` と `expire`（Go では `Balance` と `Expire`）を避け、同じ名前になる二つは後の方に `_2` を付ける。`src/client/{typescript,python,go}.rs` の `members` がこれを決め、`chobo api` の `targets` がその名前を出す。

#### 診断のコード

DESIGN 3.1 の表のとおり。E001〜E005、E010〜E013、E020〜E023、E030、E031、E040、E041、E050、E051、E060、E061、W101〜W107。増やすときは末尾から足し、番台をまたがない（E013 は段階 B で足した）。

#### コマンドの表

DESIGN 8 章のとおり。`src/main.rs` の一枚の表に、コマンドごとの目的、引数、フラグ（値の型と既定）、exit code の意味、走る例を二つ、出しうる診断のコードを持たせ、`--help` と引数の解析が同じ表を引く。知らないフラグ、閉じた集合の外の値、値の無いフラグ、二度書いたフラグは exit 2。引数なしの `chobo` は全体の `--help` を標準エラーに出して exit 2。`build` は段階 C で、`doc` は段階 D で表に足した。

### 0.4 段階の終わりにすること

1. `cargo build` に警告が無い。`cargo test` が通る。`cargo test -- --nocapture` の SKIP の行を数え、理由を書き留める。
2. DESIGN.md を、実装で分かったことに合わせて直す（決めたこと、捨てた形、実際の出力と数）。PLAN.md の、済んだ段階の終わりの条件に、実際の件数を書き足す。
3. 立てたサーバーが残っていないこと（`pgrep -fl 'tigerbeetle start'`、`pgrep -fl postgres` で、テストのものが無いこと）、一時ディレクトリが残っていないことを確かめる。

## 1. 段階 B：言語の芯

### B1 プロジェクト

`Cargo.toml`（`name = "chobo"`、`version = "0.1.0"`、`edition = "2024"`、`serde_json = { version = "1", features = ["preserve_order"] }`、`[profile.release] strip = true`）。`src/lib.rs` が各モジュールを `pub` で出し、テストから使う。ライセンスは作者が決めるので、段階 D で `license` を外した（段階 B では `MIT OR Apache-2.0` と書いていた）。

### B2 字句（syntax.rs）

- UTF-8。BOM、タブ、結合文字（U+0300〜U+036F、U+3099、U+309A）を含む名前は E001。
- 行ごとに読み、字下げ（空白の数）でブロックを作る。ブロックの行はそろった字下げで、見出しより深い。
- トークン：名前（一文字目は `char::is_alphabetic` か `_`、二文字目から英数字か `_`）、数（`-` を付けてよい。`.` のあとに桁を書ける）、文字列（`"…"`、`\"` と `\\` だけ）、`(` `)` `:` `,`、キーワード。`#` から行末はコメント。
- キーワードは `pub const KEYWORDS` の一枚の表に置く（DESIGN 1.6 の表）。名前にキーワードは使えない（E001）。ただし時間の語は `expires after <数>` のあとでだけキーワードとして読み、ほかでは名前に使える（段階 B で決めた。DESIGN 1.6）。

### B3 構文（parse.rs）

```
book      := "book" NAME VERSION NL [ "description" STRING NL ] { unit | account | transfer }
unit      := "unit" NAME [ "scale" INT ] NL
account   := "account" NAME [ "(" params ")" ] ":" NAME [ "outside" ] NL [ INDENT { aline NL } DEDENT ]
aline     := "description" STRING
           | "at" "least" NUMBER "refused" "as" NAME
           | "at" "most" NUMBER "refused" "as" NAME
transfer  := "transfer" NAME "(" params ")" NL INDENT { tline NL } DEDENT
tline     := "description" STRING
           | "key" NAME { "," NAME }
           | "pending" ( "expires" "after" INT DURATION | "never" "expires" )
           | "move" AMOUNT "from" ACCOUNT "to" ACCOUNT
params    := NAME ":" NAME { "," NAME ":" NAME }        # 型は string か単位の名前
ACCOUNT   := NAME [ "(" ARG { "," ARG } ")" ]            # ARG は振替の引数の名前か文字列
AMOUNT    := NAME | NUMBER
DURATION  := "second" | "seconds" | "minute" | "minutes" | "hour" | "hours" | "day" | "days"
VERSION   := "v" 数字
```

勘定の引数は位置で渡す（`在庫(sku)` は、振替の引数 `sku` を勘定の一つ目の引数に）。構文木のどの節点にも、行・列・長さを持たせる。

### B4 名前と型（model.rs）

解決した帳簿の型（`Book`、`Unit { name, scale }`、`AccountKind { name, params, unit, outside, lower: Option<Bound>, upper: Option<Bound>, description }`、`Bound { value: i128, refusal: String, span }`、`TransferKind { name, params, key, pending: Option<Expiry>, moves }`、`Move { amount, from, to, span }`）を作り、E002〜E005、E010、E011、E013、E020〜E023、E030、E031、E040、E041 を出す。リテラルは単位の `scale` で整数に直す（`12.50` は scale 2 で 1250）。引数の無い勘定を `()` 付きで宣言したら E001、`scale` は 0 から 18 まで（外れれば E001）。

### B5 参照インタプリタ（interp.rs）

DESIGN 2 章のとおり。

- 状態はテナントごと。勘定の posted・held_in・held_out は `i128`。額は 0 以上 2⁶³ − 1 以下で、外れれば入力の誤り（拒否ではない）。
- 操作：`do`・`hold`・`post`・`void`（結果は `done`・`done_before`・`refused(理由)`）、`pass(秒)`（期限を越えた仮押さえを期限切れにして戻す）、`status`、`balance`。
- 境界の確かめ方（2.2）、キー（2.3。名前空間、中身、境界で拒否されたキーだけを使ったことにする、確定の中身は全額を押さえた額に読み替えて比べる）、ライフサイクル（2.4 の表）、拒否の理由（2.7 の表）を、そのまま書く。
- 時刻はシナリオの始まりからの秒。`hold` のときの時刻に有効期限を足した時刻（期限）になっていれば（ちょうども含む）、`post` と `void` は `expired`。`pass` は、期限に達した仮押さえをその場で期限切れにする。
- `do` と `hold` は、`same_account`、キー、移動の境界の順に確かめる（DESIGN 2.3）。
- 動かしたあとの残高が 64 ビットの整数の範囲を出る操作は失敗（`Err`）で、何も変えない（DESIGN 1.5）。

### B6 シナリオと結果（scenario.rs）

0.3 の JSON を読み書きする。`together` は、呼び出し元ごとの順序を保ったすべての並べ方（`interleavings`）を数え上げ、それぞれを参照インタプリタで流し、結果を重ねずに集める（呼び出し元が三つ以上でも、操作が合わせて 8 までなら数え上げる。それを超えるシナリオは読むときにエラーにする）。同じ結果になる並べ方は一つにまとめ、`run_json` は 0.3 の形（`together` の結果は `callers` の中）で出す。

### B7 シナリオの生成（scenarios.rs）

DESIGN 6 章の種類を、帳簿から決まった順に作る。どのシナリオにも `name`（種類と、何を試すか。0.3）を付ける。段階 B では、`witness.rs` の `Builder` が参照インタプリタで一つずつ呼びながらシナリオを組み立て、名前のとおりの結果にならなかったものは捨てる。種類ごとの並びは、境界、キー、仮押さえ、期限切れ、移動、同時、`same_account` の順。

- **入れる振替を探す**：勘定の種類 X について、X へ入れる移動を持ち、額が引数の振替の種類を探す（すぐに確定する振替を先に。仮押さえの振替なら押さえてから全額で確定する）。その振替のほかの移動も X に触るなら、X に残る額が求める額になるように、渡す額を一次式で求める。ほかの移動の元が中の勘定で、下限があるなら、その勘定にも同じように入れる（深さ 3 まで。一つの呼び出しが同じ勘定から取る額は、まとめて一度に入れる）。見つからなければ、X に残高を作るシナリオは作らない（検査の W102 がそれを言う）。
- **額の選び方**：まず、ほかと重ならない小さな額を入れる。それで呼び出しが通らない（または言いたい移動で拒否されない）ときは、額の引数に 1、2、3、5、10、100、1000 を組み合わせて試す（違う値の組を先に、和の小さい順に）。前の移動が入れた額を後の移動が取る振替は、ここで見つかる。何も入れずに済む組み合わせを、入れる振替を呼ぶ組み合わせより先に選ぶ。
- **境界**：その移動の勘定が同じ振替のほかの移動に出てこないときだけ作る。下限 L のある勘定 X から取る移動を持つ振替の種類 K ごとに、X に `max(L, 0) + 2` を入れてから、K を、取ったあとに L + 1・L・L − 1 が残る額で呼ぶ三本。上限 U のある勘定へ入れる振替ごとに、`U − 2` まで入れてから 1・2・3 を入れる三本（U < 2 なら、入れずに U − 1・U・U + 1）。
- **キー**：振替の種類ごとに、同じ引数の二度目（`done_before`）、額の引数（無ければキーに入らない引数）だけを変えた二度目（`key_conflict`）。境界で拒否されうる種類には、拒否されたあと同じ引数で二度目（`already_refused`）、残高を足してから三度目（`already_refused`）。
- **仮押さえ**：仮押さえの振替ごとに、全額の確定、一部の確定（押さえた額が 2 以上なら 1 を確定し、残りが戻ったことを終わりの残高で見る）、取消、確定のあとの取消、取消のあとの確定、押さえた額を超える確定（`over_hold`）のあと正しい額で確定、無い仮押さえの確定（`no_such_hold`）のあと仮押さえを作って確定（キーを使っていないので `done`）、同じ額での確定の二度目と違う額での二度目。有効期限のある種類には、期限を越えて `pass` したあとの確定と取消（`expired`、残高は戻っている）、期限切れのあとに同じキーで押さえ直す（`done_before`、残高は変わらない）。
- **`pass`**：そのシナリオで作ったすべての仮押さえの期限を越える長さ（帳簿の一番長い有効期限 + 1 分）にする。突き合わせはこれを「すべての仮押さえの期限を越えるまで待つ」と読む（C5）。
- **移動が二つ以上ある振替**：そういう振替の種類ごと、境界で拒否されうる i 番目の移動ごとに、それより前の移動は通り、i 番目の移動で拒否されるように残高を作って呼ぶ。終わりの残高が呼ぶ前と同じことを見る。
- **同時**：下限のある勘定 X から取る振替の種類 K ごとに、X にちょうど一回分を入れ、二つの呼び出し元が違うキーで K を呼ぶ `together`。上限のある勘定へ入れる種類ごとに、空きがちょうど一回分のところへ二つが入れる `together`。二つの呼び出しは X の引数を同じにし、キーを変える（キーが X の引数だけでできていれば作らない）。参照インタプリタの結果が、片方が通り片方が拒否される二通りでなければ捨てる。
- **`same_account`**：元と先が同じ種類の勘定になる移動を持つ振替ごとに、引数を同じ値にそろえて呼び、`same_account` で拒否される一本。
- **値**：文字列の引数は `<引数の名前>-<番号>`（番号は、できあがったシナリオの中で最初に出てくる順に振り直す）、額は境界から決めたもののほかは、シナリオの中で違う値にする。
- 同じ帳簿からは、いつも同じシナリオを同じ順で作る。`chobo scenarios <book> --out <dir>` は `001.json` から書き、`--out` が無ければ JSON の配列を標準出力に書く。

### B8 検査と報告（check.rs、witness.rs）

- DESIGN 3.1 の全部のコード（E060 と E061 は段階 C の `build` で出すので、ここでは作らない）。
- 操作の列：W101（その勘定へ入れる振替の呼び出し。そのあと取る振替が無いことを言う）、W102（その振替を額 1 で呼んで拒否される）、W103（下限なら空の勘定から取る額を `1 − L` 以上に、上限なら空の勘定へ上限を超える額か、上限まで入れてから 1 を入れて、合わせれば収まる額で呼んで拒否される）、W104（仮押さえを呼んで拒否される）、E012（引数を同じ値にして呼んで `same_account`）。どれも参照インタプリタで流し、言うとおりの結果でなければ出さない（テストでは、出さなかったことを失敗にする）。操作の列の値の番号は、列の中で最初に出てくる順に振り直し、振り直した列をもう一度流した結果を見せる。
- 報告（DESIGN 3.2）：振替の種類と操作ごとに、拒否されうる理由と、それぞれの短い操作の列。境界の理由は移動が触る勘定から候補を作り、B7 の部品で列を組み、参照インタプリタで確かめたものだけを載せる。キーについては、キーに入らない引数ごとの `key_conflict` の例と、仮押さえの種類ごとの「終わったあとに同じキーで押さえ直すと `done_before`」の例。
- テキストでは、診断のあとに `<file>: ok`（エラーがあれば `<file>: N error(s), M warning(s)`）と、報告を理由の名前だけで書く。`--format json` は報告を例まで書く。

```json
{"v": 1, "files": [{"file": "…", "ok": true, "diagnostics": [ … ],
  "report": [{"kind": "引当", "op": "hold",
              "refusals": [{"name": "在庫切れ", "because": "bound", "spends_key": true, "example": [ <0.3 の操作と、その結果> ],
                            "account": "在庫", "as_written": "在庫(sku)", "bound": "at least 0", "move": 1}],
              "key": {"params": ["注文", "sku"],
                      "examples": [{"about": "key_conflict", "param": "数", "operations": [ … ]},
                                   {"about": "done_before", "operations": [ … ]}]}}]}]}
```

- `because` は `bound`（境界）、`key`（`key_conflict`、`already_refused`）、`account`（`same_account`）、`hold`（`no_such_hold`、`already_posted`、`already_voided`、`expired`、`over_hold`）。`account`・`as_written`・`bound`・`move` は境界の理由だけにある。`key` は `do` と `hold` の項目だけにある。
- 例の操作は 0.3 のシナリオの操作に `result`（と `reason`）を足したもの。

### B9 診断（diag.rs、codes.rs）

- テキストの形は dandori にそろえる。`error[E020]: <ファイル>:<行>:<列>: <言うこと>`、`    <行> | <原文>`、操作の列があれば `  the operations that get there:` に続けて番号つきの操作と結果、最後に `  hint: …`。日本語は `エラー[E020]`、`警告[W101]`、`  そうなる例:`、`  ヒント: …`。
- 文面は `tr!("日本語", "English")` で隣り合わせに書き、片方だけの文面を作れないようにする（rulec の `src/i18n.rs` が手本）。
- JSON：`{"v": 1, "severity", "code", "file", "line", "column", "title", "excerpt", "operations": [ … ], "hint"}`。キーは `--lang` にかかわらず英語。
- `codes.rs` にコードごとの、題、いつ出るか、どう直すか（書き換えたあとの形まで）、走る最小の再現の帳簿、関係するコードを、英語と日本語で持たせる。`chobo explain <コード>` はそれを出し、`chobo explain --all --format markdown` は全部を Markdown で出す（段階 D で `docs/codes.md` と `docs/codes.ja.md` にする）。

### B10 ID とチェーン（ids.rs）

SHA-256（FIPS 180-4）、`enc`、0.3 のすべての役目の ID、ledger、code、定義のハッシュ、中身の JSON、0.3 のチェーンの並び（操作ごとの振替の役目、借方、貸方、額、フラグ、`pending_id`、timeout、ledger、code、`user_data`）を書く。チェーンの並びは、段階 C の `chobo run --show tigerbeetle` とクライアントのテストの元になる。

### B11 `chobo api`（api.rs）

DESIGN 5 章のとおり。上の階層は `{"v": 1, "book", "version", "source_sha256", "units", "accounts", "transfers", "machines", "ids"}`（段階 C で `targets` を足した。出力先ごとの名前とチェーンの並び。DESIGN 5）。`machines` は仮押さえの振替の種類ごとに、次の形にする（`引当` の場合）。キーの名前は dandori の `src/rulec.rs` の `machine()` が読むものと同じにする。

```json
{
  "name": "引当", "over": "引当",
  "carry": {"input": "state", "output": "next_state", "enum": "state"},
  "held": [], "never": [], "once": [],
  "states": ["held", "posted", "voided", "expired"],
  "initial": "held",
  "final": ["posted", "voided", "expired"],
  "events": ["post", "void", "expire"],
  "external": ["expire"],
  "certificate": {
    "enums": {"state": ["held", "posted", "voided", "expired"], "event": ["post", "void", "expire"],
              "reason": ["none", "already_posted", "already_voided", "expired"]},
    "types": {"state": "state", "event": "event", "next_state": "state", "refused": "bool", "reason": "reason"},
    "machine": {"table": "引当", "carry": {"input": "state", "output": "next_state"}, "axis": 1,
                "states": ["held", "posted", "voided", "expired"], "initial": 0, "finals": [1, 2, 3],
                "rows": [{"row": 1, "to": 1}, {"row": 2, "to": 2}, {"row": 3, "to": 3}, {"row": 4, "stay": true},
                         {"row": 5, "stay": true}, {"row": 6, "stay": true}, {"row": 7, "stay": true},
                         {"row": 8, "stay": true}, {"row": 9, "stay": true}]},
    "tables": [{"table": "引当", "policy": "first",
                "axes": [{"column": "event", "coords": ["post", "void", "expire"]},
                         {"column": "state", "coords": ["held", "posted", "voided", "expired"]}],
                "decides": ["next_state", "refused", "reason"],
                "rows": [
                  {"row": 1, "accepts": [[0], [0]], "produces": ["posted", "false", "none"]},
                  {"row": 2, "accepts": [[1], [0]], "produces": ["voided", "false", "none"]},
                  {"row": 3, "accepts": [[2], [0]], "produces": ["expired", "false", "none"]},
                  {"row": 4, "accepts": [[0], [1]], "produces": [null, "false", "none"]},
                  {"row": 5, "accepts": [[1], [1]], "produces": [null, "true", "already_posted"]},
                  {"row": 6, "accepts": [[0], [2]], "produces": [null, "true", "already_voided"]},
                  {"row": 7, "accepts": [[1], [2]], "produces": [null, "false", "none"]},
                  {"row": 8, "accepts": [[0, 1], [3]], "produces": [null, "true", "expired"]},
                  {"row": 9, "accepts": [[2], [1, 2, 3]], "produces": [null, "false", "none"]}]}]
  }
}
```

`never expires` の種類は、`events` と軸から `expire` を除き、`external` を空にし、行 3 と 9 を除いて、行の番号を 1 から 7 に振り直す。期限を過ぎた確定と取消（期限は過ぎたが、まだ戻っていない）は、案件から見ると期限切れの状態と同じに扱う（`expire` を外で起きるイベントとして数えれば、dandori は確定の前に期限切れでありうることを数える）。

操作ごとの拒否されうる理由は `transfers[].operations.<操作>.refusals`（B8 の報告と同じ形。`do` と `hold` には `key` も）に、dandori のタスクのエラーとして読める名前で出す。`transfers[]` には `code` と `definition`（0.3 の定義のハッシュ）も入れる。

### B12 `--diff-base`（diffbase.rs）

`git -C <帳簿のディレクトリ> show <リビジョン>:./<ファイル名>` で前の帳簿を読み、E050、E051、W107 を出す。git の答えは終了コードで読む（`rev-parse --verify` でリビジョンを、`cat-file -e` でファイルを確かめる。メッセージは利用者のロケールで変わるので読まない）。そのリビジョンにファイルが無いとき、前の帳簿にエラーがあるときは比べない（テキストに一行の注、JSON に `note`）。リビジョンが無い、git が無いときはエラー（exit 2）。帳簿の名前が変わったら W107 を一つ出し、ほかは比べない。

### B13 CLI（main.rs）

`check`、`run`（`--scenario`、`--format json`。テキストは下の形）、`scenarios`、`api`、`explain`、`--help`、`--version`。`--lang`、`CHOBO_LANG`。`run` の `--scenario` は、シナリオ一つか、その配列（`chobo scenarios` の出力そのまま）。`together` のあるシナリオは、とりうる結果を一つずつ出す。帳簿にエラーがあれば、`run`・`scenarios`・`api` は診断を標準エラーに出して exit 1。

段階 B の実際の出力（在庫の帳簿の、期限切れのシナリオ）：

```
在庫.book (pass: 引当 expires, then is posted and voided): 5 steps
  1  入荷.do(納品書: 納品書-1, sku: sku-2, 数: 2)  done
  2  引当.hold(注文: 注文-3, sku: sku-2, 数: 2)    done
  3  pass 31 minutes                               引当(注文-3, sku-2) expired
  4  引当.post(注文: 注文-3, sku: sku-2)           refused: expired
  5  引当.void(注文: 注文-3, sku: sku-2)           refused: expired
accounts:
  在庫(sku-2)  posted 2, held out 0, held in 0
  仕入先       posted -2, held out 0, held in 0
  客           posted 0, held out 0, held in 0
holds:
  引当(注文-3, sku-2)  expired
```

### B14 テストの帳簿

`tests/books/` に、少なくとも次を日本語の名前で書く（段階 D の例とは別に、言語の隅を通すためのもの）。

- 在庫：下限 0、仮押さえ（有効期限）、返品。
- ウォレット：下限 0 と上限、`never expires` の仮押さえ、外の勘定へ戻す振替。
- 安全在庫：下限が 0 より大きい勘定。
- 与信：下限が 0 より小さい勘定と上限（余裕と空きの両方が要る）。
- 分配：移動が三つある振替（同じ勘定に入れてから取る正しい順序）、リテラルの額の移動、単位の違う二つの移動（両替）、`scale 2` の単位。
- 自分あて：一つの移動の元と先が同じ種類で引数が違う勘定（`same_account` が起きうる）。

`tests/fixtures/` には、診断のコードごとに少なくとも一つ、それを出す帳簿を置く（W101〜W104 は、例の操作の列が golden に入る）。

### 段階 B の終わりの条件

1. `cargo build` に警告が無く、`cargo test` が通る。段階 B のテストは外の道具を使わない（git だけ。無ければ `--diff-base` のテストが SKIP）。
2. `tests/check.rs`：`tests/fixtures` のどの帳簿も、英語と日本語の golden と一字も違わない。DESIGN 3.1 の全部のコード（E060、E061 を除く）が、どれかの golden に、両方の言語で出る。
3. `tests/cli.rs`：どのコードも `chobo explain` に英語と日本語の文と走る再現があり、再現の帳簿を検査すると、そのコードが出る。コマンドの表の `--help`、知らないフラグの exit 2、引数なしの exit 2。
4. `tests/semantics.rs`：DESIGN 2.2〜2.7 の決まりを一つずつ、手で書いたシナリオと期待する結果で確かめる（少なくとも：下限のちょうど・超える、上限のちょうど・超える、入ってくる仮押さえを数えない、出ていく仮押さえを数える、移動の順序、全部か無し、`done_before`、`key_conflict`、`already_refused` と三度目、確定の全額と一部と `over_hold`、確定と取消の組み合わせの五つ、`no_such_hold` のあと作って確定、期限のちょうど・過ぎた、期限切れのあとの押さえ直し、`same_account`、`together` の数え上げの件数）。
5. `tests/scenarios.rs`：`tests/books` のどの帳簿でも、B7 の種類のうち当てはまるものがどれも一本以上あり（名前で確かめる）、二度作って同じで、どのシナリオも参照インタプリタで流せる。全部のシナリオの結果を golden（`tests/books/<名前>.runs.json`）に固定する。
6. `tests/ids.rs`：SHA-256 の既知の値（空、`abc`、448 ビットの文、`a` を百万）と、0.3 の ID の表の全部。`tests/books` のどの帳簿でも、全シナリオの全操作のチェーンの並びを golden に固定する。
7. `tests/api.rs`：`tests/books` の `chobo api` を golden に固定し、`machines` が B11 の形であること（dandori の `machine()` の読むキーがそろっていること）、どの振替の種類と操作にも拒否されうる理由があることを確かめる。
8. `tests/diffbase.rs`：一時ディレクトリに git のリポジトリを作り（`git init`、作者の設定に頼らず `-c user.name=… -c user.email=…` でコミット）、境界・キー・移動を変えて E050・E051・W107 が出る。
9. `tests/books` のどの帳簿も `chobo check` でエラーが無い（警告は、帳簿にそれを試すと書いたものだけ）。
10. DESIGN.md の 1.1 の帳簿が `chobo check` を通り、3 章の診断と報告のスケッチを実際の出力に差し替えてある。

段階 B の結果（2026-10-03）：どの条件も満たした。`cargo build` に警告は無く、`cargo test -- --nocapture` は 36 件が通り、SKIP の行は 0（git があるので `--diff-base` のテストも走る）。内訳は `tests/api.rs` 3 件、`check.rs` 4 件、`cli.rs` 6 件、`diffbase.rs` 1 件、`ids.rs` 4 件、`scenarios.rs` 2 件、`semantics.rs` 16 件。

- `tests/fixtures`：診断の帳簿 19 冊（と、`--diff-base` の比べる側 1 冊）、golden 38 本。E060 と E061 を除く 26 のコードが、両方の言語の golden に出る。
- `tests/books`：在庫、ウォレット、安全在庫、与信、分配、自分あての 6 冊。どれも検査で何も言われない。作るシナリオは順に 21、29、9、24、17、25 本の計 125 本で、その結果（`.runs.json`）、TigerBeetle に送る 340 の操作のチェーン（`.chains.json`）、`chobo api`（`.api.json`）を golden に固定した。
- 0.3 の約束のうち、段階 B で決めたこと（シナリオの名前と値の番号、`together` の結果の形、`accounts` に並べる勘定、チェーンの勘定と開始の振替の `code` と `user_data`、定義のハッシュの文、中身の JSON の書き方）は 0.3 に書き足した。

## 2. 段階 C：PostgreSQL と TigerBeetle

### C1 `chobo run --show`（render.rs）

`--show tigerbeetle` は、操作ごとに、送る勘定（作るもの）、開始の振替、チェーン（0.3 の並び）を JSON で出す（`--format json`。テキストでは表にする）。`post` と `void` には、先に読む仮押さえの振替の ID（`lookup`）も出す。仮押さえが無ければ `sent` は null、押さえた額を超える確定なら、状態を読むチェーン（DESIGN 4.2。ID は `random`）を出す。`--show postgres` は、操作ごとに呼ぶ SQL と引数（額は 10 進の文字列、渡さない額は null）を出す。どちらも、クライアントが実際に送るものと一字ずつ比べる元になる。`together` の操作は、`together` の前の状態から求める（`ids::scenario_chains` と同じ）。

### C2 PostgreSQL（postgres.rs）

`chobo build --target postgres` が SQL を一つ書く（DESIGN 4.1、0.3 の名前）。書く先は `--out <dir>`（既定はいまのディレクトリ）の `<帳簿の名前>.sql`。

- `create schema if not exists`、`create table if not exists`、`create or replace function`。何度流してもよい。
- `accounts`（`tenant text, id uuid, kind text, args jsonb, unit text, lower_bound bigint, upper_bound bigint, posted bigint, held_in bigint, held_out bigint`、主キー `(tenant, id)`、DESIGN 4.1 の CHECK 二つ）。`holds`（`tenant, id uuid`（`hold` のチェーンの位置 0 の ID）、`kind, key jsonb, moves jsonb, state text, created_at timestamptz, deadline timestamptz, posted jsonb`）。`keys`（`tenant, kind, op, key jsonb, content jsonb, definition text, result text, reason text, at timestamptz`、主キー `(tenant, kind, op, key)`）。`entries`（`seq bigserial, tenant, at, kind, op, key jsonb, account uuid, d_posted, d_held_in, d_held_out`）。
- 関数の流れは DESIGN 4.1。`do` と `hold` は、ロックした勘定の残高を配列に読んで移動を確かめ、全部通ってから書く（例外のブロックとサブトランザクションを使わない）。`post` と `void` も `keys` に行を書き、確定済み・取消済みのリトライを、その行の中身と定義のハッシュで答える。ID は `chobo_id`（`sha256(bytea)` で 0.3 と同じに求める）。勘定の行の境界が帳簿と違えば `raise exception using errcode = 'CB001'`。
- `expire(p_max)` は、期限を過ぎた `held` の仮押さえを期限、テナント、ID の順に `for update skip locked` で取り、その勘定をテナントと ID の順にロックしてから戻し、`expired` にする。戻した数を返す。
- スキーマ、関数、引数の名前が 63 バイトを超えれば E061 でビルドを止める。

### C3 クライアント（client/）

`chobo build --target postgres-typescript|postgres-python|postgres-go|tigerbeetle-typescript|tigerbeetle-python|tigerbeetle-go`（0.3 のクライアントの名前）。

- 書く先は `--out <dir>` の下に、TypeScript は `<帳簿の名前>.ts`、Python は `<帳簿の名前>.py`、Go は `<パッケージ>/book.go` と `<パッケージ>/runtime.go`。PostgreSQL 用と TigerBeetle 用で、名前も引数も結果も同じにする（0.3）。
- 帳簿ごとに違う部分（型、呼び方、帳簿の定義の表）を生成し、どの帳簿でも同じ部分は `src/client/runtime/` のファイルをそのまま入れる。TypeScript と Python は一つのファイルの中に、Go は `runtime.go` として。
- PostgreSQL のクライアントは、関数を一つ呼び、結果の型を読む。シリアライズの失敗（40001）とデッドロック（40P01）は、同じ引数で 10 回までリトライし、あいだに 0 から 1、2、4 … 128 ミリ秒までのランダムな時間だけ待つ。二度目が 25P02（呼ぶ側のトランザクションが中断された）なら最初のエラーを返す。
- TigerBeetle のクライアントは、DESIGN 4.2 のとおり。勘定と開始の振替を先に作り（帳簿の値ごとに覚え、二度目は送らない。253 件ずつのリクエストに分ける。勘定が `exists_with_different_*` なら、勘定の定義が変わったというエラー）、`post` と `void` の前に `lookup_transfers` で仮押さえにした移動の振替（main）を全部読み（無ければ `no_such_hold`。全額の確定は、ここで読んだ額で中身を求める）、チェーンを一つのリクエストで送り、DESIGN 4.2 の表で結果を読む。開始の振替が `exists_with_different_amount` なら、境界が変わったというエラーにする。押さえた額を超える確定は送らずに、状態を読むチェーン（取消と、借方と貸方が同じ振替を linked で）を送って答える。`status` も同じチェーンで読む。残高が 64 ビットを出ていれば、`balance` がエラーにする。
- TigerBeetle のクライアントの公式のライブラリは 0.17.9（npm の `tigerbeetle-node`、PyPI の `tigerbeetle`、Go の `github.com/tigerbeetle/tigerbeetle-go`）。段階 A に、三つとも手元のレプリカに振替を送れた。Go の 0.17.9 は型がパッケージの一番上にある（`pkg/types` は無い）。cgo を使う。
- 一つの操作のチェーンが 253 件（`--development` で立てたレプリカの一つのリクエストに入る件数。DESIGN 3.1）を超えるなら E060 でビルドを止める。

### C4 道具（tools/）

- `tools/tigerbeetle/fetch.sh`：GitHub のリリースの 0.17.9 の zip を、OS とアーキテクチャに合わせて取り、SHA-256 を確かめて `tools/tigerbeetle/tigerbeetle` に置く。段階 A に求めた zip の SHA-256：

  | zip | SHA-256 |
  |---|---|
  | `tigerbeetle-universal-macos.zip` | `4e085eaffc66c2ed82e7f94a8137c468256d2fb0b35152ccf903cc6676c22940` |
  | `tigerbeetle-x86_64-linux.zip` | `af71f2c0057e3b409bf79940fa94894b738187b0d4f1e711097bca15df5d8cd4` |
  | `tigerbeetle-aarch64-linux.zip` | `413994920fe48b04f5aa86895b7a1d9d98d14803b29d4e91d2a6d0098fff4ef2` |

  段階 A でこのマシンの `tools/tigerbeetle/tigerbeetle` に 0.17.9（`TigerBeetle version 0.17.9+cc1c06a`）を置いてある。`.gitignore` 済み。
- `tools/runner/`：TypeScript（`package.json` に `tigerbeetle-node` 0.17.9 と `pg` 8.23.1、型を確かめる `typescript` 7.0.2、`@types/pg` 8.23.1、`@types/node` 26.6.4。`package-lock.json` を残す。入れ方は `npm ci --prefix tools/runner`）、Python（`requirements.in` に `tigerbeetle==0.17.9` と `psycopg[binary]==3.3.6`、`uv pip compile --generate-hashes` で作った `requirements.txt` がロック。venv は `uv venv --python 3.13 tools/runner/.venv` と `uv pip install --python tools/runner/.venv/bin/python -r tools/runner/requirements.txt`）、Go（`tools/runner/go/go.mod` に `tigerbeetle-go v0.17.9` と `github.com/jackc/pgx/v5 v5.11.0`、`go.sum`。dandori の `tools/temporal-go` と同じく、テストが全部の帳簿の生成物と `harness/` と `main.go` を一つのモジュールにして、一つのバイナリにする。`main.go` は、生成したパッケージの型つきの呼び方を `harness.Book` に合わせるもので、テストが `chobo::client::go` の名前で書く）。版は段階 C を始めるときにもう一度確かめた（どれも段階 A と同じ最新）。`node_modules` と `.venv` は `.gitignore` に足した。Go のバイナリはテストの一時ディレクトリに作る。
- ランナーの入力：`{"backend": "postgres" | "tigerbeetle", "postgres": {"host", "port", "database", "user"}, "tigerbeetle": {"cluster", "addresses"}, "books": [{"name", "client": <生成物のパスか Go のパッケージの名前>, "expiry": 3, "keys": {<種類>: [<キーの引数の名前>]}, "scenarios": [{"tenant", "steps", "accounts": [{"account", "args", "params"}], "holds": [{"kind", "key", "args"}]}]}]}`。`accounts` は終わりに読む勘定（参照インタプリタの `accounts` の順）、`holds` はシナリオの `hold` が指す仮押さえの全部（参照インタプリタの `holds` の順。無いものは出さない）。出力：`{"books": [{"name", "scenarios": [{"tenant", "result": <0.3 の形>, "sent": [{"step", "caller"?, "op", "kind", "requests": [...]}], "error"}]}]}`。`requests` は、TigerBeetle では `create_accounts`・`create_transfers`・`lookup_transfers`（勘定と振替は `chobo run --show` と同じフィールド、ID は 16 進 32 桁、額は 10 進の文字列）、PostgreSQL では `{"sql", "params"}`（額は 10 進の文字列）。クライアントに渡す接続を包んで、操作のあいだに送ったものを書き留める。
- PostgreSQL へは 20 本の接続のプールを、TigerBeetle へは四つのクライアントを使い回す（TigerBeetle のクラスタが覚えるクライアントは 64 まで）。シナリオと、`together` の呼び出し元ごとに、別の帳簿の値を作る。
- `together` は、呼び出し元ごとに別の接続（TigerBeetle では別のクライアント）を使い、同時に始める。
- `pass`：そのシナリオで作った仮押さえの期限（作った操作が返った時刻 + テストのコピーの有効期限 + 0.3 秒）を全部越えるまで眠る。TigerBeetle では、そのあと、仮押さえで借方にした勘定を読み直し、`debits_pending` が、まだ押さえ中の期限の無い仮押さえの額の合計になるまで待つ（10 秒で打ち切って失敗）。PostgreSQL では `expire()` を呼ぶ。`expire()` はほかの `expire()` が戻している仮押さえを飛ばすので、一つのランナーの中では一度に一つずつ呼ぶ。

### C5 突き合わせ（tests/backends.rs）

- テストのプロセスで一度だけ、PostgreSQL の使い捨てのクラスタ（`initdb -A trust`、ソケットは `<CHOBO_PG_SOCKET_DIR か /tmp>/chobo-pg-<pid>`（ソケットのパスは 103 バイトまで）、TCP では待ち受けない）と、TigerBeetle のレプリカ（一時ディレクトリにデータファイル、`--development`、空いているポート）を立てる。PostgreSQL のバイナリは PATH か `CHOBO_PG_BIN`（EDB のインストーラーで入れた macOS では `/Library/PostgreSQL/18/bin`。常駐のサーバーは使わず、テストのたびに使い捨てのクラスタを立てる）、TigerBeetle は `tools/tigerbeetle/tigerbeetle` か `CHOBO_TIGERBEETLE`。TigerBeetle には、テストの一時ディレクトリを `TMPDIR` として渡す（自分のコピーを 256 MB ずつそこに置くので）。サーバーのプロセス ID を一時ディレクトリの `servers` に書き、殺されたテストのプロセスが残したサーバーは、次のテストのプロセスが止めてから消す（プロセスの名前が postgres か tigerbeetle のときだけ）。
- テストのコピーとして、帳簿の `pending expires after …` の行を全部 3 秒にした帳簿から、SQL とクライアントを作る（書き換える場所が見つからなければ失敗。dandori と同じやり方）。コピーでも参照インタプリタの答えが同じであることを、全シナリオで確かめる。
- 七つの組み合わせ（SQL そのもの、PostgreSQL × 三つの言語、TigerBeetle × 三つの言語）ごとに、全部の帳簿の全部のシナリオを一つのランナーのプロセスで同時に流す（テナントは `<組み合わせ>/<帳簿>/<シナリオの番号>`）。SQL そのものは、テストが psql をシナリオと呼び出し元ごとに起こして、関数を直接呼ぶ。参照インタプリタの結果と、操作ごとの結果、終わりの残高、仮押さえの状態を比べる。`together` は `outcomes` に入ることを見る。生成したシナリオのほかに、`tests/books/<帳簿>.more.json` の手で書いたシナリオも流す。
- 送ったものを `chobo run --show` と比べる（TigerBeetle は先に読む仮押さえ、勘定、開始の振替、チェーンの全部のフィールド。PostgreSQL は SQL と引数）。三つの言語で ID が一致することは、ここで全シナリオの全操作について確かめられる。
- 最後に、PostgreSQL のどの勘定の残高も `entries` を足し合わせたものと等しいことを確かめる。
- 一行ずつ `compared: <帳簿> <組み合わせ> <件数> scenarios` を出す。道具が無いときは `SKIP:` の行。
- 負荷：同じ出力先の組み合わせは順に流し、PostgreSQL と TigerBeetle は並べて流す。全体の時間を測って DESIGN.md に書く。

### C6 PostgreSQL だけのもの（tests/postgres.rs）

DESIGN 6 章の五つ：反対の順序で同じ勘定の組に触る操作を同時に百回流してデッドロックが無いこと、REPEATABLE READ の接続で流してもリトライで同じ結果になること（三つの言語のクライアントそれぞれで、シリアライズの失敗が起きたことも数える）、関数を通らない UPDATE を CHECK が止めること、行の境界を書き換えてから操作すると `CB001` になること、`expire()` が戻した数と残高と `entries`。ほかに、0.3 の ID の表を `chobo_id` で求めて一致すること、長い名前の帳簿が E061 でビルドされないこと。

### C7 TigerBeetle だけのもの（tests/tigerbeetle.rs）

開始の振替の額が違うとエラーになること（境界を変えた帳簿を同じテナントで使う）、無い仮押さえの確定が `no_such_hold` で（読むだけで何も送らない）、そのあと仮押さえを作れば同じキーで確定できること。どちらも三つの言語のクライアントで確かめる。ほかに、移動の多い帳簿が E060 でビルドされないこと。

### C8 生成したコード（tests/generated.rs）

TypeScript を `tsc --strict --noEmit`（`tools/runner` の typescript。`erasableSyntaxOnly`、`noUnusedLocals`、`verbatimModuleSyntax` も）に、Python を `python -m py_compile` に、Go を `go vet` と `gofmt -l`（何も言わないこと）に通す。

### 段階 C の終わりの条件

1. この機械で（PostgreSQL 18、TigerBeetle 0.17.9、node、Python 3.13 の venv、go がそろって）、`cargo test -- --nocapture` が SKIP の行なしで通る。
2. `tests/books` の全部の帳簿の全部のシナリオが、七つの組み合わせで参照インタプリタと一致し、`together` のシナリオが、とりうる結果のどれかに入る。件数を DESIGN.md の 6 章に書く。
3. 全シナリオの全操作で、三つの言語のクライアントが送ったものが `chobo run --show` と一字ずつ同じ。
4. C6、C7、C8 が通る。E060 と E061 に、`chobo explain` の英語と日本語の文と走る再現があり、golden に両方の言語で出る。
5. 道具を一つずつ外して（PATH から外す、`tools/tigerbeetle/tigerbeetle` を別の名前にする）、それぞれ理由を言う `SKIP:` の行になって通ることを一度確かめる。
6. わざと一つのテストを落としても、TigerBeetle と PostgreSQL のプロセスとデータファイルが残らないことを一度確かめる。
7. DESIGN.md の 4 章の、確かめていなかったこと（`expire()` の速さ、テスト全体の時間、ランナーの負荷）を実際の数に直す。

段階 C の結果（2026-10-03）：どの条件も満たした。`cargo build` に警告は無く、`cargo test -- --nocapture` は 44 件が通り、SKIP の行は 0（PostgreSQL 18.0、TigerBeetle 0.17.9、node 23.11、Python 3.13 の venv、go 1.25.5 がそろって）。段階 B の 36 件に、`tests/backends.rs` 1 件、`postgres.rs` 1 件、`tigerbeetle.rs` 1 件、`generated.rs` 3 件、`cli.rs` の 2 件（`build` と `run --show`）を足した。全体で 43.9〜50.9 秒（最後の二回）。

- 突き合わせ：テストの帳簿 6 冊の 125 本と `tests/books/在庫.more.json` の 4 本の計 129 本が、七つの組み合わせのどれでも参照インタプリタと一致し（`together` の 11 本はとりうる結果のどれかに入った）、三つの言語のクライアントが送ったものは全部 `chobo run --show` と同じだった。組み合わせごとに 3.4〜5.4 秒、`tests/backends.rs` は 21.7〜22.0 秒。
- E060 と E061：`chobo explain` に英語と日本語の文と走る再現があり、`tests/fixtures` の 2 冊（`リクエスト.book`、`名前の長さ.book`）の golden に両方の言語で出る。診断の帳簿は 21 冊、28 のコードが全部 golden に出る。
- 道具を一つずつ外したとき（PostgreSQL、TigerBeetle、node、Python の venv、go）、それぞれ理由を言う `SKIP:` の行になって通った。`tests/backends.rs` をわざと落としたとき（`runner.py` を壊す）も、テストのプロセスを `kill -9` で殺したあとの次のテストのプロセスも、PostgreSQL と TigerBeetle のプロセス、データファイル、ソケットを残さなかった。
- 計画から変えたこと：E060 の上限を 8189 件から 253 件に（DESIGN 3.1）。組み合わせを六つから七つに（SQL そのものを psql で流す）。ランナーは帳簿ごとではなく組み合わせごとに一つ。PostgreSQL の関数は、移動を例外のブロックで戻す形をやめ、ロックした値で確かめてから書く形に（DESIGN 4.1）。TigerBeetle のクライアントの、状態を読むチェーンと、押さえた額を超える確定の扱い（DESIGN 4.2）。`chobo api` に `targets` を足した。

## 3. 段階 D：doc、例、README、スキル

### D1 `chobo doc`（doc.rs）

Markdown。DESIGN 7 章の五つ（勘定の表、振替の種類の表、勘定のあいだの流れの図、仮押さえのライフサイクルの図、シナリオ）。図は Mermaid（流れは `flowchart LR`、ライフサイクルは `stateDiagram-v2`）。読み手は経理と運用の人なので、TigerBeetle と PostgreSQL の中身（余裕と空きの勘定、チェーン）は出さない。`--lang ja` で日本語。

### D2 HTML（draw.rs）

`--format html` で一つの HTML。chobo が描く SVG（流れの図とライフサイクルの図）、表、シナリオ（一つ選ぶと操作を一つずつ進め、操作ごとの残高を見せる。残高は参照インタプリタで求めて JSON でページに入れる）。外のスクリプト、フォント、画像を読まない。明るい配色と暗い配色（`prefers-color-scheme`）。幅の狭い画面でも横にはみ出さない。

### D3 例（examples/）

四つ、英語の版と日本語の版（`<名前>.ja.book`）を並べる。どれも `chobo check` を警告なしで通り、C5 の突き合わせに入る。

- `examples/inventory/`：在庫の引当。入荷、引当（仮押さえ 30 分、出荷で確定、キャンセルで取消）、返品。
- `examples/points/`：ポイント。付与（返品の期間が過ぎるまで仮押さえ、`never expires`）、利用（会計のあいだの仮押さえ、15 分）、失効。
- `examples/refunds/`：返金は売上を超えない。注文ごとの「返金できる残り」の勘定、売上、返金（承認待ちのあいだの仮押さえ）。二つの返金が同時に最後の残りを取りに来るシナリオが、例のページに出る。
- `examples/marketplace/`：マーケットプレイスの手数料の分け方。買い手から店と手数料収入への二つの移動（額は rulec の規則のような呼ぶ側が計算して渡す）、店への支払い、返品の戻し。

例ごとに `chobo doc` の Markdown（`doc.md`、`doc.ja.md`）を置き、GitHub で読めるようにする。テストが、いまの出力と同じであることを確かめる。

### D4 README.md と README.ja.md

英語の README.md と、日本語で一から書き起こした README.ja.md（英語を訳したものにしない）。看板（DESIGN の冒頭の案。段階 A の報告で認められた）、芯（守る条件を勘定の境界に絞り、だから一度の書き込みで守れる）、帳簿の例、実際の診断と報告、コマンド、ターゲットの表、確かめ方（実際の件数）、インストール、例、まだやっていないこと。載せる出力は全部実際に走らせたもの。ライセンスの節は、作者がライセンスを決めた 2026-10-03 に足した（MIT OR Apache-2.0）。

### D5 docs/

`reference.md`（言語の全部）、`formats.md`（シナリオ、結果、`check --format json`、`api` の JSON）、`targets.md`（PostgreSQL と TigerBeetle に何が出るか、使う人向け）、`codes.md` と `codes.ja.md`（`chobo explain --all --format markdown` の出力そのもの）。

### D6 スキル（skills/chobo）

dandori の `skills/dandori` を手本にする。`SKILL.md`（手で書く。いつ使うか、書く・検査する・シナリオで動かす・ビルドする・doc を見せるの流れ、一ページの言語、人に聞くこと（境界の数と理由の名前、キー、有効期限、どの勘定が外か）、診断から直し方へ、ターゲット）と、`docs/` のページのコピー（`skills/sync.sh` が作り、リンクがスキルの外を指さないように書き換える）、`skills/README.md`。

### D7 テスト

- `tests/doc.rs`：例と `tests/books` の Markdown と HTML を golden に固定する。HTML に外への URL が無いこと。Mermaid を描けること（`tools/mermaid` に入れた mermaid で。無ければ SKIP）。
- `tests/docs.rs`：README.md、README.ja.md、docs の診断の抜粋が golden にそのまま含まれること、` ```book ` のブロックの行が例かテストの帳簿の行であること、コードの一覧がソースの台帳と一致すること、`docs/codes*.md` が `chobo explain --all` の出力と同じであること、キーワードの一覧が `src/syntax.rs` の `KEYWORDS` と同じであること。
- `tests/skill.rs`：コピーがページと同じであること、スキルの中のリンクがスキルの外を指さないこと、`SKILL.md` の frontmatter が Agent Skills の形に合うこと。

### D8 そのほか

`Cargo.toml` に `description` と `repository`。ライセンスは作者が決めるまで置かずにいた（計画では rulec と dandori と同じ二つを置くことにしていた）。2026-10-03 に作者が MIT OR Apache-2.0 に決めたので、`LICENSE-APACHE` と `LICENSE-MIT` を置き、`Cargo.toml` の `license`、README の節、スキルの frontmatter に書いた。`tests/skill.rs` は、frontmatter と `Cargo.toml` のライセンスが同じであることを確かめる。

### 段階 D の終わりの条件

1. 例の八つの帳簿が警告なしで検査を通り、C5 の突き合わせで全部のシナリオが一致する。
2. D7 のテストが通る。`cargo test -- --nocapture` が、この機械で SKIP の行なしで通る（Mermaid を入れていなければ、その SKIP だけ）。
3. README.md と README.ja.md の出力と数が、実際に走らせたものと同じ（`tests/docs.rs` が確かめる分と、手で走らせて確かめた分）。
4. DESIGN.md の 7 章を、実際の `doc` の形に直してある。

段階 D の結果（2026-10-03）：どの条件も満たした。`cargo build` に警告は無く、`cargo test -- --nocapture` は 63 件が通り、SKIP の行は 0（PostgreSQL 18.0、TigerBeetle 0.17.9、node 23.11、Python 3.13 の venv、go 1.25.5、Chrome、`tools/mermaid` の Mermaid 11.17.2 と 12.1.0 がそろって）。段階 C の 44 件に、`tests/doc.rs` 7 件、`docs.rs` 7 件、`skill.rs` 3 件、`cli.rs` 1 件（`doc` をリポジトリの根から走らせて例の横のページと比べる）、`scenarios.rs` 1 件（手で書いたシナリオの結果を固定する）を足した。全体で 56.7〜58.5 秒（最後の二回）、そのうち `tests/backends.rs` が 24.7〜25.3 秒（組み合わせごとに 3.6〜6.7 秒）。

- 例：四つ（在庫の引当、ポイント、返金、マーケットプレイス）を英語と日本語で 8 冊。どれも警告なしで検査を通る。生成するシナリオは 21、21、39、39、19、19、25、25 本の計 208 本で、返金の例には手で書いたシナリオを 1 本ずつ足した。テストの帳簿と合わせて 14 冊、339 本（`together` は 27 本）が、七つの組み合わせのどれでも参照インタプリタと一致した。
- `chobo doc`：Markdown（Mermaid の `flowchart LR` と `stateDiagram-v2`、シナリオは折りたたむ）と、一つの HTML（chobo が描く SVG、シナリオを一ステップずつ進めるスクリプト）。例の横に `doc.md` と `doc.ja.md` を置いた。golden は、テストの帳簿の Markdown 12 本、警告のある帳簿の Markdown 2 本、HTML 14 本。ページに出る 1074 のステップの残高が参照インタプリタと同じで、38 の Mermaid の図が Mermaid 11 と 12 で描け、Chrome で開いた HTML がデータどおりの残高と矢印を出す（二つのページの 12 ステップ）。
- 文書：README.md、README.ja.md、`docs/`（reference、formats、targets、codes、codes.ja）。`tests/docs.rs` が、文書に出力つきで書いた `chobo` のコマンド 16 本を走らせて出力を比べ、診断の抜粋、帳簿の行、相対リンク、コードとシナリオの数、キーワードの表を確かめる。`docs/targets.md` のクライアントの呼び方と出力は、使い捨ての PostgreSQL のクラスタと TigerBeetle のレプリカで、TypeScript・Python・Go から実際に呼んで取った。
- スキル：`skills/chobo/SKILL.md`（手で書いた）と、`skills/sync.sh` が作る `docs/` の四つのページのコピー、`skills/README.md`。
- 計画から変えたこと：ライセンスのファイルと README のライセンスの節を、作者が決めるまで置かなかった（2026-10-03 に MIT OR Apache-2.0 に決まり、置いた）。英語の診断の帳簿 `tests/fixtures/split.book` を足した（README.md に英語の名前の診断を載せるため）。手で書いたシナリオの結果を `<帳簿>.more.runs.json` に固定した。シナリオの名前に入る数を単位の桁にした（テストの帳簿の名前は変わらなかった）。
