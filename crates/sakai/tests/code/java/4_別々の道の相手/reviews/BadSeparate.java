// Reviews call into billing, though the two go separate ways.
package reviews;

public final class BadSeparate {
  public boolean billed(String day) {
    return new billing.Invoice().due(2, null, day) != null;
  }
}
