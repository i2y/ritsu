// Ordering uses the calendar billing and delivery share as their shared kernel.
package ordering;

public final class BadKernel {
  public String promised(String day) {
    return calendars.Tokyo.nextBusinessDay(day);
  }
}
