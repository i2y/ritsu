# koyomi

ファイルの拡張子は `.cal` です。

**koyomi** では、期日を、契約や法令の言い方のまま書きます。請求日から 30 日後、休みなら次の営業日、というふうにです。`koyomi check` は、宣言した範囲のすべての日について日付を計算し、書いた条件をその一日ずつで確かめます。検査を通ったファイルだけが、TypeScript、Python、Go、Rust、SQL のコードと、どのホストからも JSON のオブジェクト一つで呼べる Wasm のモジュールになります。

koyomi の文書はリポジトリにあります：[README](https://github.com/i2y/ritsu/blob/main/crates/koyomi/README.ja.md)、[リファレンス（英語）](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/reference.md)、[生成するコード（英語）](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/generated-code.md)、[診断のコード](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/codes.ja.md)。

## 入力の値の出どころ

手書きのコードが、入力を別の場所から運ぶことがよくあります。ほかのファイルの日付、契約のフィールドなどです。入力の行の最後の `fed by` は、処理系全体の参照の書き方でその出どころを書く形で、`ritsu check` が相手のファイルに、とりうる値を尋ねます。

```console
$ cd crates/koyomi/examples
$ ritsu check late_fee.ja.cal net30.cal --lang ja
late_fee.ja.cal: ok — 2 つの条件が、支払期日 2026-02-02〜2028-12-29 の 1,062 日のすべてで成り立ちます
net30.cal: ok — 3 つの条件が、invoice_date 2026-01-01〜2028-11-29 の 1,064 日のすべてで成り立ちます
ritsu check: ファイル 2 個（koyomi 2）。どれも検査を通りました。言語の境目: 確かめた 1 か所、決められない 0 か所
```

支払期日を一日縮めて `<=2028-12-28` にすると、E207 で止まります。`koyomi "net30.cal" date due は、invoice_date = 2028-11-29 のとき 2028-12-29 になります。`

確かめるのは日です。手書きのコードが本当にその値を運ぶかは何も確かめず、ページにもそう書きます（詳しくは[リファレンス](https://github.com/i2y/ritsu/blob/main/crates/koyomi/docs/reference.md#fed-by--where-an-inputs-value-comes-from)）。

## 二つの版を比べる

`koyomi diff` は、二つの版の答えを比べます。カレンダーなら両方の版が知っている日の全部で、日付のファイルなら両方の版が受け取る入力の全部で比べます。次は、事務所のカレンダーに創立記念日の休みを足し、休みの名前を一つ変え、2027 年も知るようにし、時差を持たせた版です。

```console
$ cd crates/koyomi/tests/diff
$ koyomi diff 事務所.cal 事務所_二版.cal --lang ja
koyomi diff：カレンダー 事務所 v1 → v2
  古い版：事務所.cal
  新しい版：事務所_二版.cal

比べた日：2026-01-01〜2026-12-31（365 日。両方の版が知っている日）

開いているか閉じているかが変わった日（1 区間、1 日）：
  2026-06-10：開いている → 閉じている（創立記念日）

閉じたままで、理由だけが変わった日（1 区間、1 日）：
  2026-05-04：みどりの日 → 春の休み

知っている日：2026-01-01〜2026-12-31 → 2026-01-01〜2027-12-31
時差：なし → +09:00

2026-01-01〜2026-12-31 のほかの日は、どれも同じ答えです。
```

それぞれの側には、パスのほか、git の版のファイル（`calendars/事務所.cal@origin/main` なら、そのブランチにあるままのファイル）、lock が固定する版の依存のファイル（`@calendars/事務所.cal`）、置き場にある依存のほかの版のファイル（`@calendars@1.0.0/事務所.cal`）を渡せます。exit code は、違いが無ければ 0、あれば 1、比べられなければ 2 です。比べられないのは、カレンダーと日付のファイルを渡したときや、どちらかが `koyomi check` を通らないときです。`--format markdown` は同じことを Markdown で書き、`--format json` はすべての区間を出します。
