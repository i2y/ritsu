// A stand-in for what `koyomi build` writes from calendars/東京の営業日.cal. Billing and delivery
// share it as a shared kernel.
package calendars;

public final class Tokyo {
  private Tokyo() {}

  public static String nextBusinessDay(String day) {
    return day;
  }
}
