import Kaname.order_bill_ja

namespace Proofs.order_bill_ja

open Kaname Kaname.order_bill_ja

/-- 単価を和の外に出す。前置きの `sum_mul_left` で示す。 -/
theorem bill_is_the_price_times_the_quantities : Kaname.order_bill_ja.bill_is_the_price_times_the_quantities := by
  intro lines price _ _ _ _ _
  unfold bill
  exact sum_mul_left price lines _

end Proofs.order_bill_ja
