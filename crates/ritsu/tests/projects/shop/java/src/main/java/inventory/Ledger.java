// A stand-in for what `chobo gen` writes from inventory/inventory.book: the ledger of stock. A call answers
// "done", "done_before" or "refused" with its reason (null when there is none), as chobo's Result does.
package inventory;

import java.util.HashMap;
import java.util.Map;

public final class Ledger {
  public record Result(String result, String reason) {}

  private record Held(String sku, int count) {}

  private static final Map<String, Held> HOLDS = new HashMap<>();

  private Ledger() {}

  public static Result hold(String line, String sku, int count) {
    Held before = HOLDS.get(line);
    if (before != null) {
      return before.equals(new Held(sku, count)) ? new Result("done_before", null) : new Result("refused", "key_conflict");
    }
    if (count <= 0) return new Result("refused", "out_of_stock");
    HOLDS.put(line, new Held(sku, count));
    return new Result("done", null);
  }

  public static Result post(String line) {
    return HOLDS.containsKey(line) ? new Result("done", null) : new Result("refused", "no_such_hold");
  }
}
