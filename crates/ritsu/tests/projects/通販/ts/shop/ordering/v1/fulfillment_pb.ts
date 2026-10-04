// A stand-in for the module protoc-gen-es writes from proto/shop/ordering/v1/fulfillment.proto.
// The proto imports warehouse/v1/stock.proto, so the module imports the one written from that.
import type { ReserveResponse } from "../../../warehouse/v1/stock_pb.ts";

export interface FulfillResponse {
  reservations: ReserveResponse[];
  trackingNumber: string;
}
