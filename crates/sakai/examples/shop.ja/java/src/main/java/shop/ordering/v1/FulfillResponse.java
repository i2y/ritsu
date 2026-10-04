// A stand-in for what `protoc --java_out` writes from proto/shop/ordering/v1/fulfillment.proto.
// The proto imports warehouse/v1/stock.proto, so the class uses the one written from that.
package shop.ordering.v1;

import java.util.List;
import warehouse.v1.ReserveResponse;

public record FulfillResponse(List<ReserveResponse> reservations, String trackingNumber) {}
