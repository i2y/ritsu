// A stand-in for what `chobo build` writes from inventory/在庫の引当.book: the ledger of stock.
package inventory;

public final class Ledger {
  private Ledger() {}

  public static boolean hold(String order, String sku, int count) {
    return !order.isEmpty() && !sku.isEmpty() && count > 0;
  }
}
