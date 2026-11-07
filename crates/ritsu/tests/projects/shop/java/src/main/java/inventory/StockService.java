// The stock service answers a reservation with what the ledger says, and posts a hold when the order ships.
package inventory;

import warehouse.v1.ReserveResponse;
import warehouse.v1.Stock;

public final class StockService {
  public ReserveResponse reserve(String lineId, String sku, int quantity) {
    Ledger.Result r = Ledger.hold(lineId, sku, quantity);
    if (!r.result().equals("refused")) {
      return new ReserveResponse(sku, Stock.STOCK_SECURED, lineId);
    }
    if ("key_conflict".equals(r.reason())) {
      return new ReserveResponse(sku, Stock.STOCK_CONFLICT, null);
    }
    return new ReserveResponse(sku, Stock.STOCK_SHORT, null);
  }

  public void post(String id) {
    Ledger.Result r = Ledger.post(id);
    if (r.result().equals("refused")) {
      throw new IllegalStateException("the hold " + id + " cannot be posted: " + r.reason());
    }
  }
}
