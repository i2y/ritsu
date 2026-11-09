import Kaname.media_mail

namespace Proofs.media_mail

open Kaname Kaname.media_mail

/-- A cart over 240 ounces takes the branch of `none`: the rule's answer is never read, so the proof
needs nothing of it. -/
theorem nothing_heavier_than_15lb_is_quoted : Kaname.media_mail.nothing_heavier_than_15lb_is_quoted := by
  intro inner _ lines _ _ _ heavy
  have light : ¬ weight_of lines ≤ 240 := by omega
  simp [quote, light]

end Proofs.media_mail
