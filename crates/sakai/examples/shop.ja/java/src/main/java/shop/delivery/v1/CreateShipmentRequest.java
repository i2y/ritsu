// A stand-in for what `protoc --java_out` writes from proto/shop/delivery/v1/shipment.proto.
package shop.delivery.v1;

public record CreateShipmentRequest(String region, int parcels) {}
