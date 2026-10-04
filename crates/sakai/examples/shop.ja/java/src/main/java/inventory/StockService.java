// The stock service answers a reservation with what the ledger says.
package inventory;

import warehouse.v1.ReserveResponse;
import warehouse.v1.Stock;

public final class StockService {
  public ReserveResponse reserve(String order, String sku, int quantity) {
    if (Ledger.hold(order, sku, quantity)) {
      return new ReserveResponse(sku, Stock.STOCK_SECURED, order + "/" + sku);
    }
    return new ReserveResponse(sku, Stock.STOCK_SHORT, null);
  }
}
