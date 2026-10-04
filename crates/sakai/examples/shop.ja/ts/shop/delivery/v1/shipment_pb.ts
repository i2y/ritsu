// A stand-in for the module protoc-gen-es writes from proto/shop/delivery/v1/shipment.proto.
export enum Handling {
  STANDARD = 0,
  FRAGILE = 1,
}

export interface CreateShipmentRequest {
  region: string;
  parcels: number;
}
