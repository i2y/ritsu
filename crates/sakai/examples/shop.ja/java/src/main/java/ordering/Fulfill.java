// Ordering conforms to inventory: it takes the warehouse's types as they are.
package ordering;

import java.util.List;
import shop.ordering.v1.Order;
import shop.ordering.v1.OrderStatus;
import warehouse.v1.ReserveResponse;
import warehouse.v1.Stock;

public final class Fulfill {
  public boolean shortOf(List<ReserveResponse> answers) {
    return answers.stream().anyMatch(a -> a.stock() == Stock.STOCK_SHORT);
  }

  public boolean conflicting(List<ReserveResponse> answers) {
    return answers.stream().anyMatch(a -> a.stock() == Stock.STOCK_CONFLICT);
  }

  public Order received(String id, long amountJpy) {
    return new Order(id, OrderStatus.ORDER_STATUS_RECEIVED, amountJpy);
  }

  public Order cancelled(Order order) {
    return new Order(order.id(), OrderStatus.ORDER_STATUS_CANCELLED, order.amountJpy());
  }
}
