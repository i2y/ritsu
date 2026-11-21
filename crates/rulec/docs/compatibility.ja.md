# 互換性

1.0.0 から、rulec の版の番号は [Semantic Versioning](https://semver.org/lang/ja/) に従います。このページは、rulec が読み書きするものそれぞれについて、「互換」が何を指すかを書きます。1.x のリリースが守るべきことを、ここで確かめられるようにするためです。

短く言えば、**1.0 で `rulec check` を通る規則は、どの 1.x でも通り、意味も変わりません。生成コードの呼び方も答えも変わりません。**

## どの 1.x でも変えないもの

1. **`.rule` の言語。** 1.0 が受け付ける書き方は、書き方も意味もそのまま残ります。ステートマシン（`machine`・`held`・`never`・`once`・`scenario` と、状態のほかに持ち越す値の書き方：二行目からの `carry`・`initial <入力> <値>`・`always`）、列挙の値どうしの前提のつながり（`prerequisites` と、その下の `start`・`requires`・`nothing`・`ordered by`）、いくつもの並びと、それをたどる何本もの `count`・`sum`・`max`・`min`・`fold`（たどる順は読むものから決まる）、`->` で名前を付けた fold の答え、呼び出し側のオブジェクトから読むもの（並びのフィールドの合計・最大・最小を読む `from sum`・`from max`・`from min` と、並びをそのまま取る `elements … from`）、乱数で引く入力と答えの確率の主張（`random`・`chances` と、重みで選ぶ `take_weighted`・`draw`）、ファイルを合わせる書き方（開いた値の `open … combine`、寄与のファイルの `contribution`・`to`・`after`・`into`・`extend`、開いた表・節・列挙・塊、固定の `signature:` と `meaning:`）、契約が宣言する Connect のサービスとして規則を出す書き方（`implements proto`）、規則が計算する値に受け付ける範囲を書く書き方（`define … range`。前の版はどれもこの行を読まず、構文の誤りにしていました）も含みます。`meaning:` ができる前に `sha256:` で固定した `apply` の見出しは、そのまま読みます。新しい書き方が増えることはあります。
2. **名前に使える語。** 予約語、つまり E009 が名前として受け付けない 65 語（`src/kw.rs` にある一覧）は 1.0 で固定し、テストがその一覧を見張ります。後の版で言語に語が増えることはありますが、この一覧には足しません。新しい語は、名前が来ない位置でだけ読むか、規則が宣言した同じ綴りの名前に譲ります。1.0 で使えた名前は、その後も使えます。ファイルを合わせる書き方の語もそうして足しました。`open`・`combine`・`product`・`contribution`・`to`・`into`・`extend` は行の中の位置で読むので、入力に `open` や `to` という名前を付けられます。`implements` も同じで、どのブロックの外でも行の頭でだけ読むので、入力に `implements` という名前を付けられます。`never` はもとからこの一覧にあり、`machine` の外で行の頭に来て、次が `:` でなく名前のときだけ、起こらない組み合わせの行として読みます。並びの要素のフィールドには、これまでどおり `never` という名前を付けられます。
3. **生成コードの答え。** 検査を通る規則なら、どの言語の生成コードも、同じ入力にはどの 1.x でも同じ出力を返します。参照評価器も同じです。後の 1.x で過去の記録を再生しても、前と同じ答えになります。

   1.0 の前に、生成コードの答えが一つ変わります。並びをひとりでたどる fold が止まると、前はそこでループを抜けていたので、そのあとの要素の範囲と `unique` のフィールドを確かめていませんでした。どの言語の生成コードも、参照評価器が受け付けない入力に答えていました。これからは、そうした要素も確かめて、`rulec run` と同じに断ります（`rulec run` は前から断っています。§15.328）。`tests/corpus/nationwide_freight.rule` のモジュールを、fold が止まる行と、閾値が −1 の行で呼ぶと、次のようになります。

   ```python
   nationwide_freight([Element(Zone.REMOTE, 2000, 0), Element(Zone.KINKI, -1, 0)])
   # 0.55.2：0
   # 直したあと：RuleInputError: freight_rows[1].threshold is out of range: -1
   ```
4. **生成コードの呼び方。** モジュール・関数・型・フィールド・エラーの名前、やりとりする整数の意味（§10.2）、呼び出しが書く記録、`_traced`・`_record`・`_from`・`_chances` の関数など、[`rulec api`](formats.md#api) が説明するものすべてです。新しい関数が隣に増えることはあります。

   1.0 の前に、生成した Rust の呼び方が一つ変わります。エラーの文です。`RuleError::Input { what, value }` と `RuleError::Contradiction { what }` の名前とフィールドはそのままで、`what` の型が、0.55.2 の `&'static str` から `Said` になります（§15.324）。`Said` は文の語と、文が言う要素の番号を持ち、表示するところで文を組みます。要素を断るときも、文字列は作りません。rulec が Rust で書くモジュールは、並びを読まない規則のものも含めて全部がこの型を持ち、そこから組む Wasm のモジュールも同じです。文を表示する呼び出し側（`{what}`、`what.to_string()`）と、文字列と比べる呼び出し側（`what == "amount is out of range"`）は、そのまま組めます。`&str` として渡していた呼び出し側は表示した文字列を渡し、文を書いたパターンはガードにします。

   ```rust
   // 0.55.2：what は &'static str
   Err(RuleError::Input { what: "amount is out of range", .. }) => "413 too large".to_string(),
   Err(RuleError::Input { what, .. }) => bad_request(what),
   // 直したあと：what は Said
   Err(RuleError::Input { what, .. }) if what == "amount is out of range" => "413 too large".to_string(),
   Err(RuleError::Input { what, .. }) => bad_request(&what.to_string()),
   ```

   あとの二行は 0.55.2 のモジュールでも組めるので、呼び出し側を先に直せます。

   列挙から作る名前が、ある言語で規則の名前と同じになる規則（自分の列挙 `band` を持つ `rule band`、`rule band_label`）は、その言語で列挙の型に番号を付けて `Band2` と書きます（§15.326）。`rulec api` もそう言います。0.55.2 は、そうした規則のその言語のコードを、組めないか読み込めない形か、一方の名前が他方を隠す形で書いていました。規則の関数の名前は変えないので、動いていた呼び出しは、そのまま動きます。
5. **コマンドライン。** コマンド、フラグ、その値、終了コード（0 は注記だけ、1 は指摘あり、2 は引数の誤りか読めないファイル）。コマンドやフラグが増えることはあります。
6. **診断コード。** コードの意味は変えません。使わなくなったコードは、廃止したと書いて台帳に残し、その番号を別のものに使い回しません。新しいコードが増えることはあります。新しい警告が増えても、検査を通っていた規則は通ったままです（規則に書いた鍵を言う W901 はこうして足したもので、ritsu のどの言語でも同じ意味です）。1.0 の前に、意味を変えた番号が二つあります。規則が二つ以上の並びをたどり、一つの並びを二回たどれるようになったときです（§15.219）。E031 は、並びを二回たどる必要があることを言っていましたが、一つの項目が二つの並びの要素を読むことを言うようになりました。E020 は、`elements` が二本あることを言っていましたが、たどるものの無い並びを言うようになりました。E044 は 1.0 の前に、言うことが狭くなりました。準用する規則を準用することも、並びをたどる規則や自分の列挙を出す規則を準用することもできるようになったときです（§15.230）。E044 は、準用が輪になっていること、元の規則が読めないこと、元の規則が `check` を通らないことを言い、ほかの三つで止めていた規則は通るようになりました。E033 と E040 は、寄与が足した値と寄与の固定についても、取り込んだ値と準用の固定と同じことを言います（§15.228、§15.230）。
7. **機械が読む形式。** `--format json` の出力すべて、ベクタ、fixtures、replay のマニフェスト、`adapter/1` と `extract/1` のやりとり、証明書、MCP のツールとその引数とリソース、GitHub Action の入力、リリースのアーカイブとパッケージの名前（Homebrew の `i2y/tap/ritsu`、`.deb` と `.rpm`）は、1.0 で持っているフィールドを、同じ意味のまま持ち続けます。1.x でフィールドが増えることはあり、文書が並べている値の集合に値が増えることもあります（新しい診断コード、`diff` の `domain` の新しい `what` など）。新しい書き方が、それを書いたところでだけ、フィールドに新しい形を持ち込むこともあります。入力を足し合わせる `constraint` は、証明書でも `preconditions` でも、`left` か `right` が名前のリストになるただ一つの場所です。`meaning:` で固定した `apply` は、`applies` のその項目の `sha256` が空になり、`meaning` が隣に付くただ一つの場所です。**読む側は、知らないキーを読み飛ばし、知らない値も知らないキーと同じように扱ってください。** `v` を持つ形式では、`v` がその形の版です。`check` は 2、証明書は 1 で、`v` の無い出力は 1 です。証明書を再検査するプログラムは、自分の知らない `v` を受け付けません。`tools/recheck.py` と `proofs/` の Lean のプログラムはそうしています。知らない理由も受け付けません。次の版の証明書は、上の表の行についての葉に、0.55.2 の再検査が知らない理由を五つ書くことがあり（`earlier`、`misses`、`reach`、`writes`、`disjoint`。[formats.md](formats.md)）、0.55.2 の再検査はその葉を断ります。

   次の版は、`explain` の JSON を、ritsu のどの言語でも一つの形で書きます。コードが一つなら、そのコードの字下げした一つのオブジェクト、二つ以上か `--all` なら、渡した順に並べた字下げした一つの配列です（[formats.md](formats.md)）。0.55.2 の `rulec explain --format json` と `geas explain --json` は一行に一つのオブジェクトを書いていたので、出力を一行ずつ読んでいた読み手は、全体を一つの JSON として読むことになります。オブジェクトのキーは変わりません。`explain` が `--format json` を受け付けなかった（終了コード 2）koyomi、chobo、yuen、sakai も、同じ形で書きます。

   ```text
   $ rulec explain --all --format json                    # 0.55.2
   {"code":"E001","severity":"error","title":"Unterminated string",…}
   {"code":"E002","severity":"error","title":"Unreadable character",…}
   …
   $ rulec explain --all --format json                    # 次の版
   [
     {
       "code": "E001",
       "severity": "error",
       "title": "Unterminated string",
   …
   ```

<!-- crates.io は保留中（DESIGN §15.158）。クレートを出したら、7 の名前の並びの `i2y/tap/ritsu` の後に「クレートの `rulec`」を戻す。 -->

## 1.x で変わりうるもの

- **文面。** 診断の見出し・注記・ヒント、`--help`、テキストの報告、`rulec doc` が書くもの、この文書です。読むのはコードと JSON のキーにして、文を読まないでください（§11 原則 5）。

  次の版では、受け付けない理由が並びの要素なら、文がその要素を言います。理由が一つの要素なら、フィールドの前に要素の場所（入力の配列の番号で、0 から数える）を書き、二つの要素が一緒に作る理由なら、文のあとに二つの場所を前から書きます。悪い要素が二つ以上あれば、どの言語も最初の要素を言います。参照評価器、各言語の生成コードとその runner、MCP のサーバー、Connect のサービス、ritsu のほかの言語が呼ぶ口が、同じことを言います（§15.324）。

  ```text
  $ echo '{"lines":[{"quantity":1,"price":100},{"quantity":10,"price":1000001}]}' | rulec run order_line_subtotals.rule --input -
  refused (input): subtotal is out of range            # 0.55.2
  refused (input): lines[1].subtotal is out of range   # 直したあと
  $ echo '{"lines":[{"quantity":1,"price":100},0]}' | rulec run order_line_subtotals.rule --input -
  refused (input): an element of lines is not an object   # 0.55.2
  refused (input): lines[1] is not an object              # 直したあと
  $ echo '{"buildings":[{"building":"temple"},{"building":"library"},{"building":"temple"}]}' | rulec run province_learning.rule --input -
  refused (input): buildings has two elements with the same building                                  # 0.55.2
  refused (input): buildings has two elements with the same building (buildings[0] and buildings[2])  # 直したあと
  ```

  日本語の文も同じで、`lines[1].subtotal が範囲の外です`、`buildings の要素に building が同じものが二つあります（buildings[0] と buildings[2]）` になります。

  取り出し関数（`<別名>_from`。Python・TypeScript・JavaScript・Ruby・PHP にあり、Go・Swift・Java・Rust・SQL・NumPy・Wasm には無い）も、要素を言います。呼び出し側のコレクションの要素から読む値は、パスにその要素の番号を入れて言い、その番号は、同じ要素を規則の入口が言う番号と同じです。コーパスの `order_handling_fee_from` を、二つ目の明細を二通りに誤らせて呼ぶと、次のようになります。

  ```text
  quantity "abc": order.lines[].quantity is not an integer: 'abc'    # 0.55.2
                  order.lines[1].quantity is not an integer: 'abc'   # 直したあと
  quantity 500:   quantity is out of range: 500                      # 0.55.2
                  lines[1].quantity is out of range: 500             # 直したあと
  ```

  `rulec check` が契約について言う文（E120、E121、E122）は、これまでどおり要素の中のパスを `order.lines[].sku` のように `[]` で書きます。呼び出しの一つの要素ではなく、契約がどの要素にも置く場所のことを言う文だからです。

  受け付けない理由の種類（`input` と `contradiction`）と、文の横に運ぶ値は変わりません。こうした文を文字列と比べていた呼び出し側は、新しい文と比べてください。ベクタも変わります。`vectors/<別名>.refused.jsonl` の行は新しい文を言い、一つの要素についての入力は、入口が受け付ける要素のあとに悪い要素を二つ置きます（[formats.md](formats.md#vectors-rulec-vectors-generatedvectorsaliasjsonl)）。`rulec gen --check` は、ベクタを書き直すまで落ちます。`rulec fixtures` と `rulec replay` が記録をまとめる理由は、これまでどおり要素のフィールドを並びと一緒に `lines[].amount` と言います。

  次の版では、悪い所が二つ以上ある入力にも、どの口も一つのことを言います。一つの順で最初に当たる物です。まず、関数が引数を取る順に、引数を読んで型を確かめます。次に入力の範囲、`constraint`、並びの上限を確かめ、そのあと関数が求める物を、要素ごとに確かめます（[reference](reference.md#an-input-with-two-things-wrong)、§15.328）。参照評価器と、別の順で読んでいた口は、そうした入力の一部に前と違う理由を言います。歩きが範囲の外の要素より先に矛盾に会う入力では、断りの種類も変わります。

  ```text
  $ echo '{"freight_rows":[{"row_zone":"kinki","threshold":0,"row_fee":0},{"row_zone":"kinki","threshold":0,"row_fee":0},{"row_zone":"kinki","threshold":-1,"row_fee":0}]}' | rulec run nationwide_freight.rule --input -
  refused (input): threshold is out of range                                                                     # 0.55.2
  refused (contradiction): fold verdict: two elements matched a take_unique (freight_rows[0] and freight_rows[1])   # 直したあと
  ```

  悪い所が一つの入力には、前と同じ文を言います。`vectors/<別名>.refused.jsonl` には、規則の入口が確かめる所の組ごとに、悪い所を二つ持つ入力が増えるので、`rulec gen --check` はベクタを書き直すまで落ちます。ritsu のほかの言語が規則を呼ぶ口も、口の言い方で同じ順に言います。この口は、int64 で持てない整数を、ほかの口と同じに読む所で断るようになりました。前は、範囲の端を書いていない側では通していました。
- **生成コードの本文。** 並べ方、コメント、ローカル変数の名前、そして書いた rulec の版が入るヘッダです。rulec を上げたら `rulec gen` をもう一度走らせ、変わったものをコミットしてください。それまでは `rulec gen --check` が落ちます。CI では一つのリリースを決めて入れます（`uses: i2y/ritsu@v1.0.0`）。`@v1` のように動くタグは用意しません。こちらが何も変えていないのに、生成物が古くなってしまうからです。

  次の版は、コードのコメントと、コードが返す文を、`--lang` の言語で書き、無ければ英語で書きます。`RITSU_LANG` と言語ごとの変数（`KOYOMI_LANG` など）が変えるのは、コマンドが出す文だけです。コマンドが人やプログラムのために書いて残すほかのファイル（`doc` のページ、`api` の JSON、`ritsu ringi init` が書くプロジェクト）も同じです。`ritsu gen`（パッケージの規則も含みます）と、各言語のコマンド（単独で走らせる `rulec gen` も含みます）が、この形になります。日本語のコメントにするために `RITSU_LANG=ja` を設定していたプロジェクトでは、コメントが英語になり、`--lang ja` を付けるまで `ritsu gen --check` が落ちます。`--lang ja` を付ければ、0.55.2 と同じ物を書きます。

  ```text
  $ RITSU_LANG=ja ritsu gen . --target go                 # 0.55.2
  生成しました: generated/go/rules/shipping/shipping.go
  生成しました: generated/go/units/units.go
  生成しました: generated/go/doc.go
  $ sed -n 2p generated/go/rules/shipping/shipping.go
  // もと: shipping.rule（rule shipping v1、sha256:0bb2ba9a247c926d）

  $ RITSU_LANG=ja ritsu gen . --target go --check         # 次の版（exit 1）
  生成物が古いか手で編集されています: generated/go/rules/shipping/shipping.go
  $ ritsu gen . --target go --check --lang ja             # 次の版（exit 0）
  $ RITSU_LANG=ja ritsu gen . --target go                 # 次の版
  生成しました: generated/go/rules/shipping/shipping.go
  $ sed -n 2p generated/go/rules/shipping/shipping.go
  // Source: shipping.rule (rule shipping v1, sha256:0bb2ba9a247c926d)
  ```
- **ゲートのコードが自分で使う名前と、出力先の言語の予約語。** `sekisho gen` と `ritsu gen` が書くゲートのコードは、自分の名前を宣言します。ゲートの型の名前がそのどれかと同じ綴りになると、`check` が W402 を出し、その出力先の `gen` は E401 で止まります。次の版は、そうした名前を二つ足します。一つは、action の名前でゲートに尋ねる入口（TypeScript と Python は `authorize`、`--target go` のコードは `Authorize`）です。ゲートが宣言していない action を尋ねたときの入口の誤りは、新しい種類 `action` です。TypeScript の型 `SekishoErrorKind` と、Python の種類の `Literal` に値が一つ増えるので、種類を全部挙げる `switch` には、その場合が要ります。もう一つは役割の一覧です。`roles()` はゲートが宣言した役割の全部を、`rolesOf(t)`（Python は `roles_of(t)`、`--target go` のコードは `Roles()` と `RolesOf(t)`）は型 `t` の principal が持てる役割を返します。コードは、型が持てない役割を一つでも持つ人をどの action でも拒否するので、サインインのコードは、尋ねる前にこの一覧で役割を絞ります。`--target go` のコードでは、`Authorize`・`Roles`・`RolesOf` という名前の型が W402 になり、別名を変えるまで `gen --target go` が E401 で止まります（ritsu の木のゲートには、この名前の型はありません）。

  ```text
  resource Authorize
    description "A slip that lets its holder be answered"

  $ sekisho check slips.gate
  slips.gate: ok — 1 action, 1 policy (1 permit, 0 forbids), 0 expectations, 0 separations   # 0.55.2
  warning[W402]: slips.gate:13:1: The type `Authorize` is written `Authorize` in the code of Go, a name the code uses itself   # 次の版
  $ ritsu sekisho gen slips.gate --target go
  generated: generated/go/authz/slips/slips.go           # 0.55.2
  error[E401]: slips.gate:13:1: The type `Authorize` is written `Authorize` in the code of `--target go`, a name the code uses itself   # 次の版
  ```

  次の版は、出力先の言語の予約語も同じように言います。0.55.2 は、ゲートのコードを書くどの言語でも、その予約語と同じ綴りの名前を、どこにあっても `check` の E008 で止めたので、Cedar も含めて、そのゲートのどの出力先も書けませんでした。次の版の E008 は、どのゲートでも書く Cedar の予約語だけです。出力先の言語の予約語は、その出力先のコードが名前をそのまま書く所でだけ、その出力先の W402 になります。Python のレコードのフィールド（属性）、Python の `Store` のメソッド（型を小文字にした名前）、ファイルの別名が名前になる所（Python のモジュール、`--target go` のコードのパッケージ（`_` を除いた別名）、`ritsu gen` が書くパッケージの TypeScript の索引が書き出すモジュール。こちらは `interface` のような strict モードの予約語）です。その出力先の `gen` は E401 で止まって何も書かず、ほかの出力先だけの `gen` は書きます。0.55.2 がこうした名前で止めていたゲートは `check` を通り（終了コードが 1 から 0）、`gen` は、名前をそのまま書く出力先のほかは、どれも書きます。0.55.2 が通して、読み込めないコードを書いていた二つの形は、その出力先の `gen` で止まります。小文字にすると Python のキーワードになる型（`Return`）と、`_` を除くと `--target go` のコードでキーワードになる別名（`se_lect`）です。

  ```text
  resource Parcel
    description "A parcel the depot holds"
    attributes
      type : kind

  action delete

  $ sekisho check depot.gate
  error[E008]: depot.gate:18:5: `type` is a reserved word of Go, and cannot be a name in the generated code   # 0.55.2
  error[E008]: depot.gate:20:8: `delete` is a reserved word of TypeScript, and cannot be a name in the generated code   # 0.55.2
  depot.gate: ok — 1 action, 1 policy (1 permit, 0 forbids), 0 expectations, 0 separations   # 次の版

  resource Return
    description "Goods a customer sends back"

  $ sekisho check returns.gate
  returns.gate: ok — 1 action, 1 policy (1 permit, 0 forbids), 0 expectations, 0 separations   # 0.55.2
  warning[W402]: returns.gate:13:1: The type `Return` is written `return` in the code of Python, a reserved word of the language   # 次の版
  $ ritsu sekisho gen returns.gate --target python
  generated: generated/python/authz/returns.py           # 0.55.2（読み込むと SyntaxError）
  error[E401]: returns.gate:13:1: The type `Return` is written `return` in the code of `--target python`, a reserved word of the language   # 次の版
  ```
- **落ちるべきだったのに通っていた規則。** このページや[何を証明するか](https://i2y.github.io/ritsu/rulec/ja/checks/)のページが捕まえると言っているものを、前の版が見逃していたと 1.x で分かったとき（見逃した漏れや重なり、何も宣言しないまま読み飛ばした行など）、その直しでその規則は落ちるようになります。互換のために、通ってはいけなかった規則を緑のままにはしません。こうした変更は、どんな規則に効くかを添えてリリースノートに書きます。

  次の版は、0.55.2 が通していた四種類の規則を止めます。どれも、宣言したものを答えない規則です。何も決めない出力（`->` の後ろにその出力を書く表も、同じ名前の `define` や `derive` も、`result` の行も無い出力）と、表や定義が決める所より前で読む出力と、決める `define` の式の中で読む出力は E151、見出しに `->` の無い表は E152 です。0.55.2 はそうした出力を値の無いまま答え、生成したコードは呼ぶたびに落ちるか、組めませんでした。規則が計算に使い、答えない率に書いた刻み（表の見出しや、`derive`・`define` の型に書いたもの）は、率のとる値を縛ります。刻みに載らないセルと、刻みに載らない値になりうる名前や式は E114 です。0.55.2 は、刻みと刻みの間の値のまま計算を続けていました。

  ```text
  outputs
    fee    : money[JPY]  round down(1JPY)
    review : bool

  table review_of
  policy unique
  | fee       | -> review |
  | <1000JPY  | false     |
  | >=1000JPY | true      |

  result fee = down(amount * 3%, 1JPY)

  $ rulec run read_result.rule amount=50000
  fee = 1500JPY
  review = none                                 # 0.55.2：表が、値の決まる前の手数料を読む
  $ rulec check read_result.rule
  ok read_result.rule                           # 0.55.2
  error[E151]: The output `fee` is read before it is decided   # 直したあと

  | item_kind | -> markup : rate[step 1%] |
  | books     | 0.5%                      |

  $ rulec check markup.rule
  ok markup.rule                                # 0.55.2：本の上乗せ率が 0.5% になる
  error[E114]: `0.5%` does not sit on this column's step   # 直したあと
  ```

  0.55.2 が W128（その行に当たる入力を見つけられず、当たらないことも示せなかった）と言っていた行は、当たる入力が無いことを検査が示せるようになった所で E102 になります。検査は、規則の計算する値を、行のセルの値でもう一度計算します。セルが抑える入力それぞれに数を掛けて足した形で読み、入力の刻みから決まる刻みに乗せ、`define` は規則が受け付ける範囲の中で読み、整数の上で読み、その値から別の値を決める上の表の行もたどります。そうした行のある規則は、0.55.2 では警告付きで通り、これからは落ちます。

  ```text
  define sheets : number = gifts * 2

  table note_of
  policy first
  | sheets | -> note |
  | 3      | true    |
  | -      | false   |

  $ rulec check odd_cell.rule
  ok odd_cell.rule                                     # 0.55.2（行 1 に W128）
  error[E102]: Unreachable row: row 1 never matches    # 直したあと
   `sheets` only comes to multiples of 2, and this row asks for none of them.
  ```
- **参照評価器と食い違っていた答え。** ある言語の生成コードが参照評価器と違う答えを返していたら、直して揃えます。これもリリースノートに書きます。
- **入口が通していた入力。** 参照評価器が受け付けない契約の外の入力（型の違う値、渡されていない入力、`constraint` や `never` の行が除く組み合わせ、`define` が計算した値が規則の受け付ける範囲の外になる入力など）に、ある言語の生成コードが答えていたら、直してほかの言語と同じく拒むようにします。答えていた呼び出しが、それからは落ちることがあります。入口の言う文は文面です。この変更もリリースノートに書きます。
- **書いてあるより広く読んでいた範囲。** 入口と `rulec check` が、どちらも範囲を規則に書いてあるより広く読んでいたら、直して、書いてあるとおりに読むようにします。答えていた呼び出しを、参照評価器も、どの言語の生成コードも、それからは受け付けないことがあります。検査は、書いてあるとおりの範囲の上で行を求めます。この変更もリリースノートに書きます。0.55.2 までの版は、二つの端をそう読んでいました。`>` と `<` で書いた端は、書いた値を含むものとして読み、`sum` と `count` の範囲の下の端は、歩きの終わりで比べていませんでした。

  ```text
  inputs
    amount : money[JPY]  range >0JPY <100JPY

  $ rulec run strict_input.rule amount=0
  big = false                                   # 0.55.2
  refused (input): amount is out of range       # 直したあと
  $ rulec run strict_input.rule amount=100
  big = true                                    # 0.55.2
  refused (input): amount is out of range       # 直したあと

  sum amount over lines of price  range >=1JPY <=100_000JPY

  $ echo '{"lines":[]}' | rulec run sum_lower.rule --input -
  big = none                                    # 0.55.2：網羅したと言った表に、当たる行が無い
  refused (input): amount is out of range       # 直したあと
  ```

  `>0JPY` は 1 円から、`<100JPY` は 99 円までです。値の刻み（金額と数量は単位の 1、率は宣言した刻み、日付は 1 日）の上で読みます。合計や数が、0 より大きい下の端に届かずに終わる呼び出し（空の並びなど）は、上の端を越えたときと同じ文で受け付けません。`rulec check` も同じ読み方になります。`>` や `<` の端が書いている値のための行は要らなくなり、範囲の外の値だけを取る行は E102 になり、`derive`、`max`、`min`、出力の範囲を `>` や `<` で書いていて、値がその端に届く規則は E112 か E140 になります。開いた値（`open`）の項がその端に届けば E131、`apply` で取り込む規則の入力が含まない端の値を渡していれば E043 です。契約から取る並び（`elements … from`）で、`sum` か `count` が 0 より大きい値から始まる規則は、契約がその下の端に届かないコレクション（`minItems` が無ければ空のもの）を通すなら E122 になります。契約の検証を通ったものを、生成コードが受け付けなくなるからです。範囲を `>=` と `<=` で書き、`sum` と `count` の行が 0 から始まる規則は、今までどおりに読みます。生成コードも答えも変わりません。検査で変わるのは一つだけです。`count`、`sum`、`max`、`min`、fold の答えの範囲の外の値だけを取る行は、警告（W133）でしたが、E102 になります（入力の範囲の外の行は、前から E102 です）。記録してある呼び出しのうち、これに当たるものを探すには、新しい版で `rulec fixtures lint <records.jsonl> <file.rule>` を走らせます。規則が受け付けなくなった記録を、理由ごとに挙げます（`合計 amount が宣言した範囲の下限 500JPY に届きません`）。

  同じ理由で、0.55.2 で通っていた規則があと二つ、エラーになります。どちらも、規則の入口が受け付けない呼び出しを、`rulec check` が見ていなかったものです。一つは、規則が受け付けるどの並びも届かない範囲を書いた `sum` と `count` で、E150 になります。下の端が、いちばん長い並びでどの要素も範囲の上の端のとき（`unique` のフィールドがあれば、その値を一つずつ並べたとき）の値より上にあるか、上の端が、いちばん短い並びの値より下にある範囲です。0.55.2 はこの規則を通し、下の端と比べずに答えていました。書いてあるとおりに読むと、どの呼び出しも受け付けない規則です。もう一つは契約から取る並びで、`unique` のフィールドがあるとき、または、その並びの `take_weighted` の重みの合計が、契約の通す並びで int64 を越えうるときに、E122 になります。0.55.2 の生成コードも、そうしたリクエストを受け付けていませんでしたが、`rulec check` は何も言いませんでした。

  ```text
  elements lines
    price : money[JPY]  range >=0JPY <=100JPY
    gift  : bool

  count gifts over lines where gift  range >=0 <=5
  sum amount over lines of price  range >=1_000JPY <=100_000JPY

  $ rulec check bulk.rule
  ok bulk.rule                                  # 0.55.2（amount の表のどの行にも W133）
  error[E150]: The range of the sum does not meet the values it can reach   # 直したあと
   lines has at most 5 elements (the top of the count gifts), and price is at most 100JPY in each: 5 × 100JPY = 500JPY, short of the bottom 1000JPY of this range. The generated code would refuse every call once the walk is over.
  $ echo '{"lines":[]}' | rulec run bulk.rule --input -
  big = false                                   # 0.55.2
  ```

  `rulec gen --extra vectors` が書くベクタも、この読み方に合わせて変わります。範囲に書いた端のどれにも、端ちょうどの入力が一つ入ります。合計や数から計算した値を表が読む規則には、表がその値に書いた境の両側の入力も入ります（0.55.2 では、この境の対が欠けていました）。刻みの上で計算する値に表が書いた境の両側は、その値がとる値の上に置きます。`sheets = gifts * 2` はいつも偶数なので、`<4` の両側は 2 枚と 4 枚です（0.55.2 は、どの入力からも出ない 3 枚を求めていました）。`sum` か `count` の下の端が 0 より大きい歩きの境には、その下の端の内側で入力が入り、歩きの二つの答えから計算する値の端と境にも入力が入ります。`rulec gen --check` は、書き直すまで落ちます。

  言語をまたいで `ritsu check` が確かめることも、同じに扱います。次の版は、ワークフローの呼び出しが渡す数を、ゲートが違う単位か刻みで数える組を止めます（E909）。下のタスクは `fee` を `rate[step 0.1%]` で渡すので、フローから生成したコードは 0.5% のつもりで 5 を送り、action の `input` は同じ 5 を `rate[step 1%]` の 5% として読みます。0.55.2 は、この例のようにタスクが拒まれたときのエラーを宣言していれば通し、していなければ W909 を言うだけでした。

  ```text
  task refund_order(orderId: string, fee: rate[step 0.1%] range >=0% <=10%)
    http POST orders "/orders/{orderId}/refunds"
    errors denied = 403

  flow
    refund_order(orderId: order, fee: 5)

  action refund_order                           # refunds.gate
    guards orders refundOrder
    input
      fee : rate[step 1%]  range >=0% <=10%

  $ ritsu check .
  ritsu check: 2 files (dandori 1, sekisho 1): all pass; borders between the languages: 1 checked, 0 undecided   # 0.55.2
  warning[dandori W106]: returns.flow:15:37: `5`, written for `fee`, counts steps of 0.1%: it is 0.5%   # 直したあと
  error[ritsu E909]: returns.flow:15: The task `refund_order` gives `fee` as rate[step 0.1%], and the action `refund_order` of refunds.gate reads its input `fee` as rate[step 1%]   # 直したあと
    = The two read one number as two values: the flow's 0.5% is 5% to the gate.
  ```

  横の警告は dandori のものです。次の版は、型の分かる所に単位を付けて書いた数を読み、1% でない刻みの率を整数で書いた所には、単位を付けるよう言います。`fee: 0.5%` と書いても、フローが送るのは同じ 5 で、`ritsu check` は警告なしに同じ E909 を言います（0.55.2 は、そこの `0.5%` を読みませんでした。E001 です）。
- **二つの版を同じと言っていた `diff`。** 同じ入力を受け付けない二つの版や、同じ答えを返さない二つの版に、`rulec diff` が「どの入力でも答えは同じ」と言っていたら、直して、変わったところを挙げ、終了コードを 1 にします。この変更もリリースノートに書きます。0.55.2 までの版は、要素のフィールドが受け付けるもの（範囲、型、`unique`）だけが違う二つの版と、どの表も読まない `count` や `sum` を読む式だけが違う二つの版（`define big_sale : bool = total_fee > 500` を `> 600` に変えたもの）に、終了コード 0 でそう言っていました。前のほうは `domain` に挙げます（`field_range` など。種類は [formats.md](formats.md) にあります）。後のほうは、両方の版が受け付ける並びを `count` や `sum` の値で分けて、答えを比べます（`total_fee >=501 <=600` で `big_sale: true → false`）。`--fixtures` を付けたとき、新しい版が要素のフィールドを理由に受け付けない記録は、フィールドを並びの名前と一緒に `lines[].price` と書きます（形式の説明のとおりです。0.55.2 は `price` と書いていました）。
- **入口が拒んでいた契約の中の入力。** ある言語の生成コードか参照評価器が、契約が許す入力（小数点や指数で書いた整数の値の数、`1000.0` や `1e3`）を拒んでいたら、直して受け付けるようにします。落ちていた呼び出しが、それからは答えられることがあります。この変更もリリースノートに書きます。
- **検査にかかる時間。** ただし、既定の予算の中で終わっていた規則は、その後も終わります。

## 約束の外にあるもの

- **Rust のライブラリ**（`src/lib.rs`）。モジュールが公開されているのは、テスト・ritsu（コマンドとブラウザで試すページ）・`rulec mcp` から使うためで、外に向けたインタフェースではありません。ツールと一緒に変わります。使うのは、コマンドライン、`rulec mcp`、生成コードにしてください。
- **ritsu のブラウザで試すページと、サイト。**
- **`experiments/` の下のすべて。**
