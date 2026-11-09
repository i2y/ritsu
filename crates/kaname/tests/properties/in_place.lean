import Kaname.in_place

namespace Proofs.in_place

open Kaname.in_place

theorem twice_is_even : Kaname.in_place.twice_is_even := by
  intro x _ _
  unfold twice
  omega

end Proofs.in_place
