// A stand-in for what `protoc --java_out` writes from proto/shop/ordering/v1/order.proto.
package shop.ordering.v1;

public record Order(String id, OrderStatus status, long amountJpy) {}
