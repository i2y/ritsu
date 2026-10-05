# ネットショップ

HTTP とイベントでやりとりする、小さなネットショップの四つのサービスを、sakai の地図（`ネットショップ.ctx`）に書いた例です。地図が照らし合わせるのは、それぞれのサービスの契約で、HTTP の API は OpenAPI の文書に、送ったり受けたりするイベントは AsyncAPI の文書に書いてあります。
英語の名前で書いた同じ例が [`../webshop`](../webshop/README.md) にあります。二つは同じ文書と同じ関係を持ち、境界を越える参照も診断も同じになります。

```console
$ sakai check examples/webshop.ja/ネットショップ.ctx --lang ja
examples/webshop.ja/ネットショップ.ctx: ok — コンテキスト 4、関係 5。成果物 9 件は、どれも一つのコンテキストに属する。境界を越える参照 7 件を確かめた（openapi 1、asyncapi 5、dandori 1）
```

文書は、`.proto` と同じく成果物です。sakai は、いちばん上に `openapi` か `asyncapi` があるファイルを文書として見つけ、どの文書もちょうど一つのコンテキストに属します。
境界を越える参照になるのは、文書に書いてあるものです。ほかのコンテキストの文書を指す `$ref` と、ほかのコンテキストのチャネルに送ったりそこから受けたりする AsyncAPI の操作です。
地図がワークフローを含むので、`ritsu sakai`（同じ名前のリンクの `sakai` でもかまいません）で確かめます。`sakai doc examples/webshop.ja/ネットショップ.ctx --lang ja` が地図のページを書き、[`tests/golden/doc/ネットショップ.ja.md`](../../tests/golden/doc/ネットショップ.ja.md) がそのページです。

## コンテキスト

| コンテキスト | 持ち主 | 持っているもの |
|---|---|---|
| 受注（`ordering`） | 受注チーム | `ordering/api/ordering.json`（OpenAPI 3.1.0）、`ordering/events/ordering.yaml`（AsyncAPI 3.0.0）、`common/money.yaml`（文書の一部） |
| 決済（`payments`） | 決済チーム | `payments/api/payments.yaml`（OpenAPI 3.2.0）、`payments/events/payments.yaml`（AsyncAPI 3.1.0） |
| 配送（`shipping`） | 配送チーム | `shipping/api/shipping.yaml`（OpenAPI 3.0.3）、`shipping/acl/payments.yaml`（AsyncAPI 3.0.0。腐敗防止層） |
| 通知（`notifications`） | 顧客対応チーム | `notifications/events/notifications.yaml`（AsyncAPI 3.0.0）、`notifications/notify.flow`（dandori） |

受注、決済、配送は、自分の文書を公表された言語（`ordering.v1`、`payments.v1`、`shipping.v1`）にし、だれでも使ってよい HTTP の操作（`operationId` で書く）とチャネルを、公開ホストサービスに並べています。

## 関係

| 関係 | パターン | 境界を越えるもの |
|---|---|---|
| 決済 → 受注 | 順応者、`through ordering.v1` | 決済は `orderPlaced` を受け取ります。決済の文書のチャネルが、受注のチャネルを `$ref` で指しています |
| 配送 → 決済 | 腐敗防止層、`through payments.v1`、層は `shipping/acl` | 層の文書が `paymentSucceeded` と `paymentFailed` を受け取ります。課金の状態 `ChargeStatus` は、値ごとに配送の `ShipmentGate` に読み替え、`failed` と `refunded` は拒否します |
| 通知 → 受注 | 順応者、`through ordering.v1` | 通知は `orderPlaced` と `orderCancelled` を受け取り、通知のワークフローは受注の OpenAPI の文書を読みます（`use openapi`） |
| 受注と決済 | 共有カーネル：`common/money.yaml` | 決済のスキーマが `Money` を `$ref` で使います |
| 通知と決済 | 別々の道 | 何も越えません。sakai はそれを確かめます |

## 変えたときに出る診断

決済がチャネル `paymentFailed` を開くのをやめると、そこから受け取っている配送の層が、検査を止めます。

```text
エラー[E210]: shipping/acl/payments.yaml:11:5: 「配送」の shipping/acl/payments.yaml が、「決済」の公開ホストサービスでないチャネル paymentFailed を使っています（receive）
    11 |     $ref: '../../payments/events/payments.yaml#/channels/paymentFailed'
  = 公表された言語 payments.v1 の公開ホストサービスは createCharge、getCharge、paymentSucceeded です。
  = 境界の向こうのチャネルに送ったりそこから受けたりできるのは、相手が `open host service` に並べたチャネルだけです（HTTP の操作も同じです）。相手の公表された言語の `open host service` に足してもらうか、相手が開いたものを使ってください。
  関わるもの:
      配送  shipping/acl/payments.yaml:11     $ref: ../../payments/events/payments.yaml#/channels/paymentFailed
      決済  payments/events/payments.yaml:14  #/channels/paymentFailed（公表された言語 payments.v1 のもの）
```

`tests/mutants` のうち、名前が `E108_文書の字下げのタブ`、`E108_AsyncAPI_2_の文書`、`W104_URL_を指す_ref`、`E202_公表されていない文書`、`E204_層の外の文書`、`E205_公表された言語に上流の課金`、`E206_別々の道を越えるイベント`、`E210_開いていないチャネル`、`E301_文書に無いチャネル`、`E401_課金の状態に値が増えた` のものが、この例を一か所ずつ変えたものです。

## 文書について

文書は、この例のために書きました。
2026-10-06 に手で、Redocly CLI 2.58.1（`redocly lint --extends minimal`）が三つの OpenAPI の文書を正しいと言い、AsyncAPI の parser 3.6.3（`@asyncapi/parser`）が四つの AsyncAPI の文書を誤りなく読みました。どちらもテストでは走らせません（テストはネットワークを使いません）。
受注の OpenAPI の文書は、dandori が読む（`use openapi`）ので JSON で書きました。ほかは YAML で、sakai は ritsu の、JSON と行き来できる YAML の読み手で読みます。
