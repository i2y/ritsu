import Kaname.order_flow

namespace Proofs.order_flow

open Kaname Kaname.order_flow

/-- A case for each state: an open one contradicts the `when` line, and a closed one refuses
whatever the event. -/
theorem a_closed_order_takes_no_event : Kaname.order_flow.a_closed_order_takes_no_event := by
  intro order event _ _ closed
  cases order <;> simp_all [«open», next]

end Proofs.order_flow
