import Kaname.media_mail_ja

namespace Proofs.media_mail_ja

open Kaname Kaname.media_mail_ja

/-- 240 オンスを超えるカートは `none` の枝に進む。規則の答えを読まないので、証明は規則について
何も要らない。 -/
theorem nothing_heavier_than_15lb_is_quoted : Kaname.media_mail_ja.nothing_heavier_than_15lb_is_quoted := by
  intro inner _ lines _ _ _ heavy
  have light : ¬ weight_of lines ≤ 240 := by omega
  simp [quote, light]

end Proofs.media_mail_ja
