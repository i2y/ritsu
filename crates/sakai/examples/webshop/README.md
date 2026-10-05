# The web shop

Four services of a small web shop that talk over HTTP and events, written as a sakai map (`webshop.ctx`) over the contracts of the services: OpenAPI documents for their HTTP APIs, and AsyncAPI documents for the events they send and receive.
Its twin with Japanese names is [`../webshop.ja`](../webshop.ja/README.ja.md); the two hold the same documents and the same relationships, and give the same crossings and the same diagnostics.

```console
$ sakai check examples/webshop/webshop.ctx
examples/webshop/webshop.ctx: ok — 4 contexts, 5 relationships; 9 artifacts, each in one context; 7 crossings checked (openapi 1, asyncapi 5, dandori 1)
```

A document is an artifact like a `.proto`: sakai finds it by what its top says (`openapi`, `asyncapi`), and each belongs to exactly one context.
What crosses a boundary is what the documents write: a `$ref` to another context's document, and an AsyncAPI operation that sends to or receives from another context's channel.
The map holds a workflow, so it is checked by `ritsu sakai` (or `sakai`, the link of the same name); `sakai doc examples/webshop/webshop.ctx` writes the map's page, and [`tests/golden/doc/webshop.en.md`](../../tests/golden/doc/webshop.en.md) is that page.

## The contexts

| Context | Owner | What it owns |
|---|---|---|
| Ordering (`ordering`) | Ordering team | `ordering/api/ordering.json` (OpenAPI 3.1.0), `ordering/events/ordering.yaml` (AsyncAPI 3.0.0), `common/money.yaml` (a part of a document) |
| Payments (`payments`) | Payments team | `payments/api/payments.yaml` (OpenAPI 3.2.0), `payments/events/payments.yaml` (AsyncAPI 3.1.0) |
| Shipping (`shipping`) | Shipping team | `shipping/api/shipping.yaml` (OpenAPI 3.0.3), `shipping/acl/payments.yaml` (AsyncAPI 3.0.0, its anticorruption layer) |
| Notifications (`notifications`) | Customer care team | `notifications/events/notifications.yaml` (AsyncAPI 3.0.0), `notifications/notify.flow` (dandori) |

Ordering, Payments and Shipping publish their documents as published languages (`ordering.v1`, `payments.v1`, `shipping.v1`), and list as open host services the HTTP operations (by their `operationId`) and the channels anyone may use.

## The relationships

| Relationship | Pattern | What crosses it |
|---|---|---|
| Payments → Ordering | conformist, through `ordering.v1` | Payments receives `orderPlaced`: its document's channel is a `$ref` to Ordering's |
| Shipping → Payments | anticorruption layer, through `payments.v1`; the layer is `shipping/acl` | the layer's document receives `paymentSucceeded` and `paymentFailed`; the charge's `ChargeStatus` is mapped value by value to Shipping's `ShipmentGate`, and `failed` and `refunded` are refused |
| Notifications → Ordering | conformist, through `ordering.v1` | Notifications receives `orderPlaced` and `orderCancelled`, and its workflow reads Ordering's OpenAPI document (`use openapi`) |
| Ordering and Payments | shared kernel: `common/money.yaml` | Payments' schemas take `Money` by `$ref` |
| Notifications and Payments | separate ways | nothing, and sakai checks that nothing does |

## What a change is caught as

When Payments stops opening the channel `paymentFailed`, Shipping's layer, which receives from it, stops the check:

```text
error[E210]: shipping/acl/payments.yaml:11:5: The document shipping/acl/payments.yaml of Shipping uses the channel paymentFailed of Payments (receive), which is no open host service of Payments
    11 |     $ref: '../../payments/events/payments.yaml#/channels/paymentFailed'
  = The open host services of the published language payments.v1 are createCharge, getCharge, paymentSucceeded.
  = Across a boundary, a document sends to and receives from only the channels the other side lists under `open host service` (and the same for HTTP operations): have it listed there, or use what the other side opens.
  involved:
      Shipping  shipping/acl/payments.yaml:11     $ref: ../../payments/events/payments.yaml#/channels/paymentFailed
      Payments  payments/events/payments.yaml:14  #/channels/paymentFailed, a part of the published language payments.v1
```

The mutants of `tests/mutants` named `E108_…`, `W104_…`, `E202_a_document…`, `E204_a_document…`, `E205_the_upstream_charge…`, `E206_events…`, `E210_…`, `E301_a_channel…` and `E401_value_added_to_the_charge_status` are this example with one change each.

## The documents

The documents are written for this example.
By hand, on 2026-10-06, Redocly CLI 2.58.1 (`redocly lint --extends minimal`) found the three OpenAPI documents valid, and the AsyncAPI parser 3.6.3 (`@asyncapi/parser`) parsed the four AsyncAPI documents with no error; neither runs in the tests, which use no network.
The OpenAPI document of Ordering is JSON, as dandori reads it (`use openapi`); the others are YAML, which sakai reads by ritsu's reader of the YAML that goes to JSON and back.
