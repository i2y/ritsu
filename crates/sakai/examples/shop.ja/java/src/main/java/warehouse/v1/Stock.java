// A stand-in for what `protoc --java_out` writes from proto/warehouse/v1/stock.proto.
// The import linters look at the boundaries between packages, not at what is in them.
package warehouse.v1;

public enum Stock { STOCK_UNSPECIFIED, STOCK_SECURED, STOCK_SHORT }
