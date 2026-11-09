import Kaname.order_bill_ja

namespace Proofs.order_bill_ja

open Kaname Kaname.order_bill_ja

/-- 単価を和の外に出す。明細についての帰納法で示す。 -/
theorem bill_is_the_price_times_the_quantities : Kaname.order_bill_ja.bill_is_the_price_times_the_quantities := by
  intro lines price _ _ _ _ _
  unfold bill
  have key : ∀ ls : List Line, (ls.map (fun l => price * l.qty)).sum = price * (ls.map (fun l => l.qty)).sum := by
    intro ls
    induction ls with
    | nil => simp
    | cons l ls ih => simp [List.sum_cons, ih, Int.mul_add]
  exact key lines

end Proofs.order_bill_ja
