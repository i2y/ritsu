// A stand-in for the module protoc-gen-es writes from proto/warehouse/v1/stock.proto.
// The import linters look at the boundaries between modules, not at what is in them.
export enum Stock {
  UNSPECIFIED = 0,
  SECURED = 1,
  SHORT = 2,
}

export enum PackingStatus {
  UNSPECIFIED = 0,
  WAITING = 1,
  PACKED = 2,
  SHORT = 3,
}

export interface ReserveResponse {
  sku: string;
  stock: Stock;
  id?: string;
}
