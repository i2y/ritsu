# データベース向けのビルド

`chobo build` は、帳簿を PostgreSQL、TigerBeetle、MySQL の系統（MySQL、TiDB、Vitess）のどれか向けに書き、それを TypeScript、Python、Go、Rust から呼ぶクライアントを書きます。

| ターゲット | 書くもの |
|---|---|
| `postgres` | `<帳簿>.sql`。スキーマ、テーブルと制約、操作ごとの関数 |
| `postgres-typescript`、`postgres-python`、`postgres-go`、`postgres-rust` | その関数を呼ぶクライアント |
| `tigerbeetle-typescript`、`tigerbeetle-python`、`tigerbeetle-go`、`tigerbeetle-rust` | 操作ごとに、振替のチェーンを一つ TigerBeetle に送るクライアント |
| `mysql` | `<帳簿>.sql`。MySQL、TiDB、Vitess 向けの、帳簿の三つのテーブルと制約 |
| `mysql-typescript`、`mysql-python`、`mysql-go`、`mysql-rust` | 操作ごとに、そのテーブルの上で一つのトランザクションを行うクライアント |

```console
$ chobo build examples/inventory/inventory.ja.book --target postgres --out db
$ chobo build examples/inventory/inventory.ja.book --target postgres-typescript --out db
$ chobo build examples/inventory/inventory.ja.book --target tigerbeetle-go --out db
$ chobo build examples/inventory/inventory.ja.book --target mysql --out db
$ chobo build examples/inventory/inventory.ja.book --target mysql-python --out db
$ chobo build examples/inventory/inventory.ja.book --target tigerbeetle-rust --out db
```

どの出力先でも、一つの操作は一度の書き込みです。全部の移動を境界で確かめて全部を書くか、拒否して何も書かないかのどちらかです。確かめてから書くまでのあいだに、ほかの呼び出しが変えうるものは読みません。これができるのは、境界が一つの勘定の条件で、移動が書き換える勘定しか見ないからです。出力先ごとのくわしいことは、四つの言語から呼んだ例とその出力と一緒に、[出力先](targets.md)（英語）にあります。

## PostgreSQL

`--target postgres` は SQL のファイルを一つ書きます。何度流してもよく、無いものを作り、関数は置き換えます。帳簿はスキーマ一つになり、テーブルは四つです。

- `accounts`：勘定ごとに一行。残高と、勘定を作ったときの境界を持ちます。
- `holds`：仮押さえ。
- `keys`：使ったキーの全部と、どの引数で呼ばれ、どうなったか。
- `entries`：勘定の変化の全部。残高をあとから足し直せます。

操作は関数 `"<振替>_<操作>"` で、テナント（ほかと分けて持つ残高の一組。帳簿が一組しか持たないなら `''`）と振替の引数を受け取り、`result` と `reason` を返します。

関数は、操作が動かす勘定の行を ID の順にロックし、残高を読み、帳簿に書いた順に移動を確かめます。在庫の引当の例の `引当.hold` では、次のとおりです。

<!-- chobo build examples/inventory/inventory.ja.book --target postgres -->
```sql
  -- (3) the moves, in the order they are written, each checked against the locked rows as the
  -- moves before it left them. Nothing is written until every move has gone through.
  -- move 数 from 在庫(sku) to 客
  v_from := array_position(v_ids, v_from_1);
  v_to := array_position(v_ids, v_to_1);
  if v_posted[v_from] - v_held_out[v_from] - "p_数" < 0 then
    -- refused by a bound: nothing has moved, and the key stays used
    update "在庫の引当".keys k set result = 'refused', reason = '在庫切れ'
     where k.tenant = p_tenant and k.kind = '引当' and k.op = 'hold' and k.key = v_key;
    return ('refused', '在庫切れ')::"在庫の引当".result;
  end if;
  v_held_out[v_from] := v_held_out[v_from] + "p_数";
  v_held_in[v_to] := v_held_in[v_to] + "p_数";
```

- **READ COMMITTED で足ります。** 確かめるときに読むのは、関数がロックした行だけです。同じ勘定への二つ目の呼び出しは、一つ目がコミットするのを待ち、一つ目が残したもので確かめます。どの関数も同じ順序で行をロックするので、デッドロックは起きません。
- **呼ぶ側のトランザクション。** 関数は呼ばれたトランザクションの中で動くので、注文の行と、その在庫の仮押さえを一緒にコミットできます。クライアントはコミットもロールバックもしません。REPEATABLE READ か SERIALIZABLE では、呼び出しがシリアライズの失敗で返ることがあり、そのときクライアントは同じ引数で 30 回までリトライします。キーがあるので、二度は動きません。
- **期限切れ。** PostgreSQL には、時刻が来たら自分で動く仕組みがありません。期限を過ぎた仮押さえは、確定にも取消にもすぐに `expired` を返しますが、押さえていた量は、`expire()` が戻すまで数えられたままです。pg_cron か自前のジョブから、同時に二つ以上走らないようにして呼んでください。
- **最後の歯止め。** `accounts` の二つの `CHECK` 制約が、関数を通らない書き込みで境界が破られるのを止めます。境界が帳簿と違う行があれば、その勘定を使う呼び出しは、どれもエラー `CB001` で止まります。

## TigerBeetle

TigerBeetle は利用者のコードを中で動かさないので、仕事はクライアントがします。操作ごとに、つないだ振替のチェーンを一つのリクエストで送り、TigerBeetle はそれを全部通すか、一つも通さないかのどちらかにします。

- 帳簿の勘定は、どれも TigerBeetle の勘定になります。残高は貸方から借方を引いたもので、移動は勘定から勘定への振替です。単位は ledger になります。
- 下限 0 は、勘定のフラグ `debits_must_not_exceed_credits` で守ります。それ以外の境界は、chobo がチェーンに足す勘定と振替で守ります。0 より上の下限は、その量の仮押さえを移動のすぐあとに作って取り消すことで守り、0 より下の下限と上限は、境界までの余地を表す勘定を同じチェーンの中で動かすことで守ります。一つの移動は、多くても六つの振替になります。
- 確定と取消の前に、クライアントは仮押さえを読みます。無ければ何も送らずに `no_such_hold` を返します。仮押さえより先に確定が届くと、その仮押さえはもう確定できなくなるからです。
- チェーンは、一つのリクエストに入らなければなりません。chobo のテストで使う、`--development` で立てたレプリカでは 253 件までなので、ビルドはそこで止まります（E060）。

```console
$ chobo run examples/refunds/refunds.ja.book --scenario examples/refunds/refunds.ja.more.json --show tigerbeetle --lang ja
examples/refunds/refunds.ja.book (more: 30000 円の売上に 20000 円の返金が二つ同時に来て、そのあと 10000 円を返金する): 3 ステップ、TigerBeetle に送るもの（テナント ""）
  1  売上計上.do(注文: A-100, 額: 30000)
       勘定:
         account  売上                   3e16d4e0db113320fe6e23db1e3df523
         account  返金できる残り(A-100)  bdbc0ebfd9f7848f96aefccff5d44dd0  debits_must_not_exceed_credits
       チェーン:
         0  main  売上 → 返金できる残り(A-100)  30000  217b25d31ad2533140f25044abc9d8b2
…
```

テストの届かない二つの場合で、TigerBeetle は参照インタプリタと違う結果を返します。境界で拒否されたキーに違う額のリトライが来たときと、残高が 64 ビットに収まらなくなったときです。それぞれどうなるかは[出力先](targets.md#tigerbeetle)（英語）にあります。

## MySQL、TiDB、Vitess

`--target mysql` は、MySQL 8.4 以降、TiDB 8.5 以降、Vitess 向けの SQL のファイルを一つ書きます。何度流してもかまいません。流したデータベースの中に、帳簿の名前を頭に付けたテーブルを三つ作ります。呼ぶための関数はありません。TiDB にはストアドプロシージャが無く、Vitess を動かすサービスには、ストアドルーチンもデータベースを作ることも許さないものがあるからです。そのため、操作ごとに、chobo が書いた SQL 文をクライアントが一つのトランザクションの中で送ります。クライアントがするのは、値を入れることと、前の文の結果から次の文を選ぶことだけです。返金の例の日本語の版では、勘定のテーブルは次のとおりです。

<!-- chobo build examples/refunds/refunds.ja.book --target mysql -->
```sql
create table if not exists `返金_accounts` (
…
  `posted` bigint not null default 0,
  `held_in` bigint not null default 0,
  `held_out` bigint not null default 0,
  primary key (`tenant`, `id`),
  constraint `返金_within_lower` check (`lower_bound` is null or `posted` - `held_out` >= least(`lower_bound`, 0)),
  constraint `返金_within_upper` check (`upper_bound` is null or `posted` + `held_in` <= greatest(`upper_bound`, 0))
) default charset = utf8mb4 collate = utf8mb4_bin;
```

勘定ごとに一行で、残高と、勘定を作ったときの境界を持ち、二つの `CHECK` 制約が付きます。ほかの二つは、キーを使った呼び出しごとに一行の `返金_calls`（仮押さえの呼び出しの行が、そのまま仮押さえの行です）と、勘定の変化を全部残す `返金_entries` です。`--show mysql` を付けると、クライアントが送る SQL 文を、値を入れた形で出します。「確かめる勘定」は、帳簿の値がその勘定を初めて使うときに、呼び出しの前に行を読み、無ければ作る勘定です。次は、二つの返金が同時に来るシナリオの、最後の返金です。

```console
$ chobo run examples/refunds/refunds.ja.book --scenario examples/refunds/refunds.ja.more.json --show mysql --lang ja
examples/refunds/refunds.ja.book (more: 30000 円の売上に 20000 円の返金が二つ同時に来て、そのあと 10000 円を返金する): 3 ステップ、MySQL に送るもの（テナント ""）
…
  3  返金.hold(申請: R-3, 注文: A-100, 額: 10000)
       確かめる勘定: bdbc0ebfd9f7848f96aefccff5d44dd0, 715ef96cb340b51f2b92d9f05af0b178
       begin /*T! pessimistic */
       insert into `返金_calls` (`tenant`, `id`, `tenant_name`, `kind`, `op`, `key`, `content`, `definition`, `result`, `at`, `state`, `moves`, `deadline`) values (X'3739d5debcffee27177072cc74c874e9', X'90345989bd5cd14f54e08379ad1a5fb1', _utf8mb4 X'', '返金', 'hold', _utf8mb4 X'5b22522d33225d', _utf8mb4 X'7b22e794b3e8ab8b223a22522d33222c22e6b3a8e69687223a22412d313030222c22e9a18d223a31303030307d', '6234460e16f10a423ce994b3ecb7328d', 'done', utc_timestamp(6), 'held', '[{"from":"bdbc0ebfd9f7848f96aefccff5d44dd0","to":"715ef96cb340b51f2b92d9f05af0b178","amount":"10000"}]', utc_timestamp(6) + interval 604800 second)
       select hex(`id`), case when `id` = X'bdbc0ebfd9f7848f96aefccff5d44dd0' and `posted` - (`held_out`) - 10000 < 0 then 1 else 0 end from `返金_accounts` where `tenant` = X'3739d5debcffee27177072cc74c874e9' and `id` in (X'bdbc0ebfd9f7848f96aefccff5d44dd0') order by `id` for update
       insert into `返金_entries` (`tenant`, `call_id`, `n`, `account`, `at`, `kind`, `op`, `key`, `d_posted`, `d_held_in`, `d_held_out`, `folded`) values (X'3739d5debcffee27177072cc74c874e9', X'90345989bd5cd14f54e08379ad1a5fb1', 1, X'bdbc0ebfd9f7848f96aefccff5d44dd0', utc_timestamp(6), '返金', 'hold', _utf8mb4 X'5b22522d33225d', 0, 0, 10000, 1), (X'3739d5debcffee27177072cc74c874e9', X'90345989bd5cd14f54e08379ad1a5fb1', 2, X'715ef96cb340b51f2b92d9f05af0b178', utc_timestamp(6), '返金', 'hold', _utf8mb4 X'5b22522d33225d', 0, 10000, 0, 0)
       update `返金_accounts` set `held_out` = `held_out` + (case when `id` = X'bdbc0ebfd9f7848f96aefccff5d44dd0' then 10000 else 0 end) where `tenant` = X'3739d5debcffee27177072cc74c874e9' and `id` in (X'bdbc0ebfd9f7848f96aefccff5d44dd0')
       commit
```

- **最初にキーを確かめます。** `do` と `hold` は、まず自分の呼び出しの行を入れます。使ったことのあるキーなら主キーの重複でそこで止まり、前の呼び出しの行を読んで、`done_before`、`key_conflict`、`already_refused` のどれを返すかを決めます。
- **ロックをかけて読むのは一度だけです。** 次に、境界のある勘定の行を ID の順にロックして読みます。どの移動が最初に境界を破るかは、その読み取りの結果に出ます（`case` の部分）。書き込みまでに、ほかには何も読みません。境界の無い勘定（ここでは `返金済み`）はロックしません。そこへ入る分は記録として残し、`expire()` が勘定の行に足し込むので、別の注文の返金が互いを待つことはありません。
- **呼ぶ側のトランザクションにも入れます。** 呼び出しは、渡した接続がトランザクションの中にいればその一部になるので、注文の行と在庫の仮押さえを一緒にコミットできます。プールか、トランザクションの中にいない接続なら、呼び出しごとに自分のトランザクションを持ち、デッドロック、ロックの待ちのタイムアウト、TiDB の書き込みの衝突のあとは、同じ引数で呼び直します。mysql2 の接続には、呼び出しの前に `set @chobo = 0` を送って、トランザクションの中にいるかを聞きます。PyMySQL の接続は、サーバーの状態を手元に持っているので、聞かずに分かります。mysqlclient の接続では、autocommit を切ったものだけが呼ぶ側のトランザクションになります。一つの接続は、一度に一つの呼び出しにだけ使ってください。同時に呼ぶなら、プールを渡します。`batch` を付けると、続けて送る SQL 文を一つのリクエストにまとめ、境界を確かめる呼び出しがロックを持つあいだの往復は一回になります。
- **期限切れ。** PostgreSQL と同じく、期限を過ぎた仮押さえはすぐに `expired` を返しますが、押さえていた量は、`expire()` が戻すまで数えられたままです。自前のジョブから、同時に二つ以上走らないようにして呼んでください。
- **キーの比べ方と名前の長さ。** キーとテナントは、照合順序にかかわらず、ID のバイト列として比べます。値は 16 進で書くので、`sql_mode` にもよりません。MySQL の名前は 64 字までで、基本多言語面の外の文字（𠮷 など）を持てません。テーブルと制約の名前にできない帳簿の名前なら、ビルドはエラーで止まります（E062）。日本語の字も一字を一字と数えるので、PostgreSQL より長い名前を付けられます。
- **TiDB と Vitess。** TiDB では、`tidb_enable_check_constraint` を ON にしてから SQL を流してください。OFF のまま作ったテーブルには制約が付きません（境界はクライアントが守ります）。SQL とクライアントは、Vitess を動かすサービスが許す範囲で書いてあります。シャードを分けたキースペースでは、三つのテーブルを `tenant` で分けてください。テナントの行も、どの呼び出しも、一つのシャードに収まります。呼び出しと一緒にコミットする自分の行も、同じシャードに置く必要があります。テストは Vitess 24.0.5 の試験用のサーバー（vttestserver）で、シャード一つと二つの両方で流しています。

## クライアント

どのデータベースのクライアントも、名前も引数も結果も同じです。データベースを替えても、変わるのは帳簿の値の作り方だけです。呼び出しは `done`、`done_before`、理由つきの拒否のどれかを返します。理由は専用の型で、境界の理由が先に並びます。

<!-- chobo build examples/inventory/inventory.ja.book --target postgres-typescript -->
```ts
/** Why a call of 在庫の引当 can be refused: the bounds' own reasons, then the ones chobo gives itself. */
export type Reason = "在庫切れ" | "key_conflict" | "already_refused" | "same_account" | "no_such_hold" | "already_posted" | "already_voided" | "expired" | "over_hold";

/** What a call answers. A refusal is an answer, not an error: nothing has changed, and the reason says why. */
export type Result = { result: "done" } | { result: "done_before" } | { result: "refused"; reason: Reason };
```

Rust では理由が列挙になり、帳簿の名前はそのまま値や構造体のフィールドの名前になります。

<!-- chobo build examples/inventory/inventory.ja.book --target postgres-rust -->
```rust
/// Why a call of 在庫の引当 can be refused: the bounds' own reasons, then the ones chobo gives itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reason {
    /// `在庫切れ`
    在庫切れ,
    /// `key_conflict`
    KeyConflict,
…
pub struct 引当Args<'a> {
    pub 注文: &'a str,
    pub sku: &'a str,
    pub 数: i64,
}
```

- **拒否は例外ではなく、結果です。** 在庫が無いのは業務として起こりうる結果なので、呼ぶ側は `在庫切れ` を、ほかの結果と同じように扱います。失敗はそれとは別のもので、例外（Go では `error`、Rust では `Err`）になります。データベースにつながらない、引数の型が違う、額が範囲の外にある、といった場合です。
- **リトライしても安全です。** キーは振替の引数から作る帳簿のものなので、同じ呼び出しをもう一度すると、`done_before` を返して何も動かしません。どのサービスから、どの言語で送っても同じです。
- **帳簿の値。** TypeScript では `postgres(db, { tenant })`、`tigerbeetle(client, { tenant })`、`mysql(db, { tenant, batch })` で作ります。Python では `postgres(conn, tenant="")`、`tigerbeetle(client, tenant="")`、`mysql(conn, tenant="", batch=False)` で作り、引数はキーワード引数で渡します。Go では `Postgres(q, tenant)`、`TigerBeetle(c, tenant)`、`MySQL(q, tenant, batch)` で作り、どの呼び出しも最初の引数に `context.Context` を取ります。Rust では `postgres(db, tenant)`、`tigerbeetle(client, tenant)`、`mysql(db, tenant, batch)` で作り、どの呼び出しも `async fn` です（`book.引当().hold(引当Args { 注文: "o-1", sku: "A-1", 数: 2 })`）。額の型は、`bigint`、`int`、`int64`、`i64` です。
- **仮押さえ。** 先に押さえる振替には `hold`、`post`（全額か、渡した額）、`void`、`status` があり、どの勘定にも `balance` があります。PostgreSQL と MySQL の系統では、帳簿の値に `expire()` もあります。

クライアントが import するのは、その言語の標準ライブラリと、データベースのクライアントのライブラリ（Go の pgx と go-sql-driver/mysql、Rust の sqlx と tokio、四つの言語の TigerBeetle のもの）だけです。TypeScript と Python の、PostgreSQL と MySQL の系統のクライアントは、渡された接続を通して動きます。TypeScript は書いたまま Node.js で動き、`tsc --strict` を通ります。Python は `mypy --strict` を、Go は `gofmt` と `go vet` を通ります。Rust はエディション 2021 と 2024 の両方で、clippy の lint も含めて警告なしで組め、頭に Cargo.toml に要る行を書きます。

## ID

TigerBeetle では、どの ID も、帳簿に書いてあることから SHA-256 のハッシュで作ります。勘定の ID は、帳簿の名前、テナント、勘定の種類と、その引数から作ります。振替の ID は、帳簿の名前、テナント、振替の種類、操作、キーの値、チェーンの中の位置から作ります。だからリトライでは同じ ID が送られ、TigerBeetle はその振替がすでにあるという結果を返します。四つの言語と参照インタプリタは同じ ID を作り、テストは、どのシナリオのどの操作でもそれを確かめています。PostgreSQL と MySQL の系統でも、勘定と仮押さえは同じ ID を持ちます。MySQL の系統では、呼び出しとテナントにも同じ作り方の ID を付け、キーとテナントをバイト列として比べます。ID を自分で求めるツールのために、`chobo api` が作り方を出します（[形式](formats.md#the-api-of-a-book)、英語）。
