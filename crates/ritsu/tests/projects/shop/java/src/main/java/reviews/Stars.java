// Reviews keep to themselves: nothing goes between reviews and billing.
package reviews;

public final class Stars {
  private Stars() {}

  public static double average(int[] stars) {
    if (stars.length == 0) return 0;
    double sum = 0;
    for (int s : stars) sum += s;
    return sum / stars.length;
  }
}
