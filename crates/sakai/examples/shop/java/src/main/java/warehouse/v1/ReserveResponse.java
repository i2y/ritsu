// A stand-in for what `protoc --java_out` writes from proto/warehouse/v1/stock.proto.
package warehouse.v1;

public record ReserveResponse(String sku, Stock stock, String id) {}
