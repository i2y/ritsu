import Kaname.order_bill

namespace Proofs.order_bill

open Kaname Kaname.order_bill

/-- The price taken out of the sum, by the prelude's `sum_mul_left`. -/
theorem bill_is_the_price_times_the_quantities : Kaname.order_bill.bill_is_the_price_times_the_quantities := by
  intro lines price _ _ _ _ _
  unfold bill
  exact sum_mul_left price lines _

end Proofs.order_bill
