// A stand-in for the module protoc-gen-es writes from proto/shop/ordering/v1/order.proto.
export enum OrderStatus {
  UNSPECIFIED = 0,
  RECEIVED = 1,
  PAID = 2,
  SHIPPED = 3,
  CANCELLED = 4,
}

export interface Order {
  id: string;
  status: OrderStatus;
  amountJpy: bigint;
}
