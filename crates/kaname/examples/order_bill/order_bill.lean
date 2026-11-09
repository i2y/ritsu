import Kaname.order_bill

namespace Proofs.order_bill

open Kaname Kaname.order_bill

/-- The price taken out of the sum: by induction on the lines. -/
theorem bill_is_the_price_times_the_quantities : Kaname.order_bill.bill_is_the_price_times_the_quantities := by
  intro lines price _ _ _ _ _
  unfold bill
  have key : ∀ ls : List Line, (ls.map (fun l => price * l.qty)).sum = price * (ls.map (fun l => l.qty)).sum := by
    intro ls
    induction ls with
    | nil => simp
    | cons l ls ih => simp [List.sum_cons, ih, Int.mul_add]
  exact key lines

end Proofs.order_bill
