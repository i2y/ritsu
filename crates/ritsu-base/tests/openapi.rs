//! The operations of OpenAPI (3.0, 3.1, 3.2) and AsyncAPI (3.0, 3.1) documents. The documents in
//! `tests/fixtures/openapi/` are valid as the tools of each specification read them: Redocly CLI
//! 2.58.1 (`redocly lint --extends minimal`) finds no error in the four OpenAPI documents (one of
//! them, `注文.yaml`, written in Japanese), and AsyncAPI's own parser (`@asyncapi/parser` 3.6.3)
//! none in the two AsyncAPI documents. Where an operation starts is held to where Redocly says it
//! is (its line and column), and the fields, the content types and the channels of the AsyncAPI
//! operations to what that parser reads.

use ritsu_base::openapi::{self, Bound, Document, Field, Kind, Number, Operation, Place};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/openapi").join(name)
}

fn text(name: &str) -> String {
    std::fs::read_to_string(fixture(name)).unwrap()
}

/// A fixture read with the files beside it at hand.
fn read(name: &str) -> Document {
    let path = fixture(name).to_string_lossy().to_string();
    openapi::read_with(&path, &text(name), &|p| std::fs::read_to_string(p).ok()).unwrap()
}

fn op<'a>(d: &'a Document, key: &str) -> &'a Operation {
    d.operation(key).unwrap_or_else(|| panic!("no operation {key}"))
}

fn int(n: i128, exclusive: bool) -> Option<Bound> {
    Some(Bound { value: Number::Int(n), exclusive })
}

fn names(fs: &[Field]) -> Vec<(&str, bool)> {
    fs.iter().map(|f| (f.name.as_str(), f.required)).collect()
}

#[test]
fn an_openapi_3_0_document_in_yaml() {
    let d = read("shop.yaml");
    assert_eq!((d.kind, d.spec.as_str(), d.title.as_str(), d.version.as_str()), (Kind::OpenApi, "3.0.3", "Shop", "2.1.0"));
    let ops: Vec<(String, usize, usize)> = d.operations.iter().map(|o| (o.name(), o.line, o.col)).collect();
    // where Redocly says each operation is
    assert_eq!(ops, [("listOrders", 11, 5), ("placeOrder", 25, 5), ("GET /orders/{orderId}", 46, 5), ("noteOrder", 61, 5)].map(|(n, l, c)| (n.to_string(), l, c)));
    assert!(d.unresolved.is_empty(), "{:?}", d.unresolved);

    // a parameter of the components, and one of the operation's own
    let list = op(&d, "listOrders");
    assert_eq!((list.method.as_str(), list.path.as_str(), list.pointer.as_str()), ("GET", "/orders", "/paths/~1orders/get"));
    let limit = list.param("limit").unwrap();
    assert_eq!((limit.place, limit.required, limit.schema.ty.as_deref()), (Place::Query, false, Some("integer")));
    assert_eq!((limit.schema.minimum.clone(), limit.schema.maximum.clone()), (int(1, false), int(100, false)));
    let status = list.param("status").unwrap();
    assert_eq!(status.schema.values, Some(vec!["paid".into(), "shipped".into(), "returned".into(), "refunded".into()]));
    // the document's security, and the status codes as written
    let sec = list.security.as_ref().unwrap();
    assert_eq!((sec.requirements.clone(), sec.own), (vec![vec!["staff".to_string()]], false));
    assert!(!list.open_to_anyone());
    assert_eq!(list.responses.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["200", "default"]);

    // `security: []`, and a body of the components whose schema is an `allOf`
    let place = op(&d, "post /orders");
    assert_eq!(place.id, "placeOrder");
    assert!(place.open_to_anyone());
    let body = place.body.as_ref().unwrap();
    assert_eq!((body.required, body.media.as_str()), (true, "application/json"));
    assert_eq!(names(&body.fields), [("customerId", true), ("total", true), ("express", false)]);
    let total = place.field("total").unwrap();
    // 3.0's `exclusiveMinimum: true` leaves the minimum out
    assert_eq!((total.schema.minimum.clone(), total.schema.maximum.clone()), (int(0, true), int(1_000_000, false)));
    assert_eq!(place.field("express").unwrap().schema.ty.as_deref(), Some("boolean"));
    assert_eq!(place.responses.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["201", "4XX"]);

    // the path item's parameters, one of them taken over by the operation's own
    let get = op(&d, "GET /orders/{orderId}");
    assert_eq!(get.id, "");
    let ps: Vec<(&str, Place, bool)> = get.params.iter().map(|p| (p.name.as_str(), p.place, p.required)).collect();
    assert_eq!(ps, [("orderId", Place::Path, true), ("X-Request-Id", Place::Header, true), ("session", Place::Cookie, false)]);
    assert_eq!(get.param("X-Request-Id").unwrap().schema.format.as_deref(), Some("uuid"));
    assert_eq!(get.security.as_ref().map(|s| s.own), Some(false));

    // a requirement that asks for nothing, and a form
    let note = op(&d, "noteOrder");
    let sec = note.security.as_ref().unwrap();
    assert_eq!(sec.requirements, vec![vec!["staff".to_string(), "audit".to_string()], vec![]]);
    assert!(sec.own && note.open_to_anyone());
    let body = note.body.as_ref().unwrap();
    assert_eq!((body.required, body.media.as_str()), (false, "application/x-www-form-urlencoded"));
    let n = note.field("note").unwrap();
    assert_eq!((n.schema.ty.as_deref(), n.schema.nullable), (Some("string"), true));
}

#[test]
fn an_openapi_3_1_document_in_json() {
    let d = read("orders.json");
    assert_eq!((d.kind, d.spec.as_str()), (Kind::OpenApi, "3.1.0"));
    let ops: Vec<(String, bool, usize, usize)> = d.operations.iter().map(|o| (o.name(), o.webhook, o.line, o.col)).collect();
    // a path item of the components is read where it is written; a webhook is not under `paths`
    assert_eq!(ops, [("getOrder", false, 7, 7), ("refundOrder", false, 18, 7), ("exportRefunds", false, 60, 9), ("refundIssued", true, 45, 7)].map(|(n, w, l, c)| (n.to_string(), w, l, c)));
    assert!(d.unresolved.is_empty(), "{:?}", d.unresolved);

    let refund = op(&d, "refundOrder");
    // a `$ref` whose JSON Pointer is written with `%7B` and `%7D`
    let id = refund.param("orderId").unwrap();
    assert_eq!((id.place, id.required, id.schema.ty.as_deref()), (Place::Path, true, Some("string")));
    assert_eq!(names(&refund.body.as_ref().unwrap().fields), [("amount", true), ("reason", false), ("share", false), ("kind", false)]);
    let amount = refund.field("amount").unwrap();
    assert_eq!((amount.schema.ty.as_deref(), amount.schema.minimum.clone(), amount.schema.maximum.clone()), (Some("integer"), int(1, false), int(10_000, false)));
    // a list of types with `null`, and `null` among the values
    let reason = refund.field("reason").unwrap();
    assert_eq!((reason.schema.ty.as_deref(), reason.schema.nullable, reason.schema.values.clone()), (Some("string"), true, Some(vec!["damaged".to_string(), "late".to_string()])));
    // 3.1's `exclusiveMinimum` is a number of its own
    let share = refund.field("share").unwrap();
    assert_eq!((share.schema.minimum.clone(), share.schema.maximum.clone()), (int(0, true), int(1, false)));
    assert_eq!(refund.field("kind").unwrap().schema.values, Some(vec!["refund".to_string()]));
    let sec = refund.security.as_ref().unwrap();
    assert_eq!(sec.requirements, vec![vec!["staff".to_string()], vec!["workflow".to_string()]]);
    assert_eq!(refund.responses, [("201".to_string(), 36), ("403".to_string(), 37)]);

    let export = op(&d, "POST /refunds/export");
    assert_eq!((export.id.as_str(), export.pointer.as_str()), ("exportRefunds", "/paths/~1refunds~1export/post"));
    assert!(export.body.is_none() && export.params.is_empty());

    // a webhook is named by its id only, and its body comes from the file beside the document
    let issued = op(&d, "refundIssued");
    assert!(!issued.answers_to("POST refundIssued"));
    assert_eq!(names(&issued.body.as_ref().unwrap().fields), [("id", true), ("amount", true)]);
    assert_eq!(issued.security, None);
}

#[test]
fn a_ref_to_a_file_not_at_hand_is_listed_and_not_guessed() {
    let path = fixture("orders.json").to_string_lossy().to_string();
    let d = openapi::read(&path, &text("orders.json")).unwrap();
    let issued = op(&d, "refundIssued");
    // the body is there; what it holds is behind the `$ref` not followed
    assert!(issued.body.as_ref().unwrap().fields.is_empty());
    let un: Vec<(&str, usize)> = d.unresolved.iter().map(|u| (u.written.as_str(), u.line)).collect();
    assert_eq!(un, [("schemas.yaml#/Refund", 47)]);
    assert_eq!(d.unresolved[0].file, path);
    // a loop of `$ref`s, and one to nothing
    let looped = "openapi: 3.1.0\ninfo: {title: t, version: '1'}\npaths:\n  /a:\n    post:\n      operationId: a\n      requestBody:\n        $ref: '#/components/requestBodies/A'\n      parameters:\n        - $ref: '#/components/parameters/Gone'\n      responses: {}\ncomponents:\n  requestBodies:\n    A:\n      $ref: '#/components/requestBodies/B'\n    B:\n      $ref: '#/components/requestBodies/A'\n";
    let d = openapi::read("loop.yaml", looped).unwrap();
    let a = op(&d, "a");
    assert!(a.body.is_none() && a.params.is_empty());
    let un: Vec<&str> = d.unresolved.iter().map(|u| u.written.as_str()).collect();
    assert_eq!(un, ["#/components/parameters/Gone", "#/components/requestBodies/A"]);
}

#[test]
fn an_openapi_3_2_document_with_its_new_methods() {
    let d = read("catalog.yaml");
    assert_eq!(d.spec, "3.2.0");
    let ops: Vec<(&str, &str, &str, usize, usize)> = d.operations.iter().map(|o| (o.id.as_str(), o.method.as_str(), o.pointer.as_str(), o.line, o.col)).collect();
    assert_eq!(
        ops,
        [
            ("searchProducts", "QUERY", "/paths/~1products/query", 7, 5),
            ("linkProducts", "LINK", "/paths/~1products/additionalOperations/LINK", 21, 7),
            ("getProduct", "GET", "/paths/~1products~1{sku}/get", 27, 5),
        ]
    );
    assert_eq!(op(&d, "QUERY /products").id, "searchProducts");
    assert_eq!(names(&op(&d, "searchProducts").body.as_ref().unwrap().fields), [("text", false)]);
    // `in: querystring` is the whole query, written with `content`
    let filter = op(&d, "getProduct").param("filter").unwrap();
    assert_eq!((filter.place, filter.schema.ty.as_deref()), (Place::Query, Some("object")));
}

#[test]
fn asyncapi_operations_with_their_channels_and_messages() {
    let d = read("events.yaml");
    assert_eq!((d.kind, d.spec.as_str(), d.title.as_str()), (Kind::AsyncApi, "3.0.0", "Order events"));
    let ops: Vec<(&str, &str, &str, &str, usize, usize)> = d.operations.iter().map(|o| (o.id.as_str(), o.method.as_str(), o.path.as_str(), o.pointer.as_str(), o.line, o.col)).collect();
    assert_eq!(
        ops,
        [
            ("publishOrderPlaced", "send", "orders/{orderId}/placed", "/operations/publishOrderPlaced", 31, 3),
            // a channel with no address is named by its key
            ("onRefundRequested", "receive", "refunds", "/operations/onRefundRequested", 37, 3),
        ]
    );
    let placed = op(&d, "publishOrderPlaced");
    assert_eq!(placed.params.iter().map(|p| (p.name.as_str(), p.place)).collect::<Vec<_>>(), [("orderId", Place::Channel)]);
    // the channel's messages, the document's content type
    let body = placed.body.as_ref().unwrap();
    assert_eq!((body.required, body.media.as_str()), (true, "application/json"));
    assert_eq!(names(&body.fields), [("orderId", true), ("total", true), ("region", false)]);
    assert_eq!(placed.field("region").unwrap().schema.values, Some(vec!["uk".to_string(), "eu".to_string()]));
    assert_eq!(placed.security.as_ref().map(|s| s.requirements.clone()), Some(vec![vec!["broker".to_string()]]));
    assert!(!placed.open_to_anyone() && placed.responses.is_empty());
    // the operation's own message, its content type, a schema in a Multi Format Schema Object
    let refunds = op(&d, "onRefundRequested");
    let body = refunds.body.as_ref().unwrap();
    assert_eq!(body.media, "application/cloudevents+json");
    assert_eq!(names(&body.fields), [("amount", true)]);
    let amount = refunds.field("amount").unwrap();
    assert_eq!((amount.schema.minimum.clone(), amount.schema.maximum.clone()), (int(1, false), int(10_000, false)));
    assert!(refunds.open_to_anyone());

    let d = read("stock.yaml");
    assert_eq!((d.kind, d.spec.as_str()), (Kind::AsyncApi, "3.1.0"));
    let stock = op(&d, "receiveStock");
    assert_eq!(stock.path, "stock/{warehouse}/received");
    assert_eq!(stock.param("warehouse").unwrap().schema.values, Some(vec!["north".to_string(), "south".to_string()]));
    assert_eq!(names(&stock.body.as_ref().unwrap().fields), [("sku", false), ("qty", false)]);
    assert_eq!(stock.body.as_ref().unwrap().media, "");
    assert_eq!(stock.security, None);
}

#[test]
fn what_is_not_read_says_why_in_both_languages() {
    let cases = [
        ("a.yaml", "swagger: '2.0'\ninfo: {title: t, version: '1'}\npaths: {}\n", "Swagger 2.0", 1),
        ("a.yaml", "asyncapi: 2.6.0\ninfo: {title: t, version: '1'}\nchannels: {}\n", "AsyncAPI 2.6.0", 1),
        ("a.json", "{\"openapi\": \"4.0.0\", \"info\": {}}", "OpenAPI 4.0.0", 1),
        ("a.yaml", "name: not a document\n", "neither", 1),
        ("a.yaml", "- a list\n", "not a mapping", 1),
    ];
    for (file, src, says, line) in cases {
        let e = openapi::read(file, src).unwrap_err();
        assert!(e.message.en.contains(says), "{src}: {}", e.message.en);
        assert!(!e.message.ja.is_empty() && e.message.ja != e.message.en, "{src}");
        assert_eq!(e.line, line, "{src}");
    }
    // what the YAML reader does not read, it says where
    let e = openapi::read("a.yaml", "openapi: 3.1.0\nopenapi: 3.1.0\n").unwrap_err();
    assert_eq!(e.line, 2);
}

#[test]
fn an_operation_is_named_by_its_id_or_its_method_and_path() {
    let d = read("shop.yaml");
    assert_eq!(op(&d, "placeOrder").name(), "placeOrder");
    assert!(op(&d, "placeOrder").answers_to("POST /orders") && op(&d, "placeOrder").answers_to("post /orders"));
    assert!(d.operation("POST /orders/").is_none() && d.operation("deleteOrder").is_none());
    assert_eq!(op(&d, "GET /orders/{orderId}").name(), "GET /orders/{orderId}");
}

/// The Japanese version of a document: names, paths and properties written in Japanese are read as
/// they are, a JSON Pointer escapes the path's `/` and keeps the rest, and a column counts
/// characters (where Redocly says the operation starts).
#[test]
fn a_document_written_in_japanese() {
    let d = read("注文.yaml");
    assert_eq!((d.kind, d.title.as_str()), (Kind::OpenApi, "店の注文"));
    let refund = op(&d, "返金する");
    assert_eq!((refund.line, refund.col), (11, 5));
    assert_eq!(refund.pointer, "/paths/~1注文~1{注文番号}~1返金/post");
    assert!(refund.answers_to("POST /注文/{注文番号}/返金"));
    let id = refund.param("注文番号").unwrap();
    assert_eq!((id.place, id.required), (Place::Path, true));
    assert_eq!(names(&refund.body.as_ref().unwrap().fields), [("金額", true), ("理由", false)]);
    let amount = refund.field("金額").unwrap();
    assert_eq!((amount.line, amount.col), (27, 17));
    assert_eq!((amount.schema.minimum.clone(), amount.schema.maximum.clone()), (int(1, false), int(10_000, false)));
    assert_eq!(refund.field("理由").unwrap().schema.values, Some(vec!["破損".to_string(), "遅延".to_string()]));
    assert_eq!(refund.security.as_ref().map(|s| (s.requirements.clone(), s.own)), Some((vec![vec!["staff".to_string()]], false)));
    assert_eq!(refund.responses.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["201", "403"]);
}
