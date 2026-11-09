import Kaname.late_payment_ja

namespace Proofs.late_payment_ja

open Kaname Kaname.late_payment_ja

/-- 期日までに払われた請求書は、期限がどの日を期日にしても 1 日も遅れていない。規則には聞かない。 -/
theorem nothing_is_claimed_when_paid_on_time : Kaname.late_payment_ja.nothing_is_claimed_when_paid_on_time := by
  intro inner _ invoice paid_on reference_rate _ _ _ _ _ on_time
  have not_late : ¬ paid_on - inner.payment_terms_ja_due invoice.invoiced ≥ 1 := by omega
  simp [claim, not_late]

end Proofs.late_payment_ja
