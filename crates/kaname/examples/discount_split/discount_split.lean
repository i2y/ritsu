import Kaname.discount_split

namespace Proofs.discount_split

open Kaname Kaname.discount_split

/-- Each share is made from one running total, and the running totals keep the lines in order
(`runningTotals_items`): the rule's answers give only the amounts, never the labels. -/
theorem every_line_keeps_its_label : Kaname.discount_split.every_line_keeps_its_label := by
  intro inner _ lines discount _ _ _ _ _
  conv => rhs; rw [← runningTotals_items lines (fun (l : Line) => l.amount)]
  simp [split, List.map_map, Function.comp_def]

end Proofs.discount_split
