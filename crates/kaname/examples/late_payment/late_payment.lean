import Kaname.late_payment

namespace Proofs.late_payment

open Kaname Kaname.late_payment

/-- Paid by the day it fell due, the invoice is late by no day, whatever day the terms give: the rule
is never asked. -/
theorem nothing_is_claimed_when_paid_on_time : Kaname.late_payment.nothing_is_claimed_when_paid_on_time := by
  intro inner _ invoice paid_on reference_rate _ _ _ _ _ on_time
  have not_late : ¬ paid_on - inner.payment_terms_due invoice.invoiced ≥ 1 := by omega
  simp [claim, not_late]

end Proofs.late_payment
