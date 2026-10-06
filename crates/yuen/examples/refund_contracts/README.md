# refund_contracts

A shop's refunds, met by elements of its contracts and its policies: an operation and a property of its OpenAPI document (`api/orders.yaml`), an operation of its AsyncAPI document (`events/orders.yaml`), a policy of its Cedar policies (`policies/refunds.cedar`) and an action of its Cedar schema (`policies/shop.cedarschema`). The rules are decided for this example.

Each link names one element (`openapi "api/orders.yaml" operation refundOrder`), and its end is that element's value with the values its `$ref`s reach, or the policy and the declaration as Cedar lays them out. A change to the schema an operation takes marks the links to that operation; a change to another operation of the same document, or a comment, marks nothing.

`refund_contracts.req` is in English, over the files with English names; `refund_contracts.ja.req` is the same project in Japanese, over `api/注文.yaml`, `events/注文.yaml`, `policies/返金.cedar` and `policies/店.cedarschema`. Each `.req` is a project of its own.

`reviewed/` and the `reviewed` and `approved` lines were written by `yuen review --date 2026-10-06`.

```console
$ ritsu yuen check examples/refund_contracts/refund_contracts.req --root examples/refund_contracts
$ ritsu yuen check examples/refund_contracts/refund_contracts.ja.req --root examples/refund_contracts
```
